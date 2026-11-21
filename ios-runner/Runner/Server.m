#import "Server.h"

#import <Network/Network.h>

#import "Auth.h"
#import "Json.h"

static const uint32_t PUMaxFrame = 16 * 1024 * 1024;
static const uint32_t PUMaxFirstFrame = 4096;

@implementation PUServer {
    uint16_t _port;
    NSString *_token;
    NSMutableSet<NSString *> *_seen;
    PUHandler _handler;
    dispatch_queue_t _queue;
    nw_listener_t _listener;
    nw_connection_t _current;
}

- (instancetype)initWithPort:(uint16_t)port token:(NSString *)token handler:(PUHandler)handler
{
    if ((self = [super init])) {
        _port = port;
        _token = [token copy];
        _seen = [NSMutableSet set];
        _handler = [handler copy];
        _queue = dispatch_queue_create("dev.phonedriver.runner.server", DISPATCH_QUEUE_SERIAL);
    }
    return self;
}

// usbmuxd reaches a device's TCP ports from a non-loopback address, so a runner on a device must listen on all
// interfaces; on the Simulator, loopback is enough. This listener also takes IPv6, the family of the remote pairing's
// tunnel (seen on macOS, tests/test_server.py).
#if TARGET_OS_SIMULATOR
static const char *PUListenHost = "127.0.0.1";
#else
static const char *PUListenHost = "0.0.0.0";
#endif

- (BOOL)start:(NSError **)error
{
    nw_parameters_t params = nw_parameters_create_secure_tcp(NW_PARAMETERS_DISABLE_PROTOCOL, ^(nw_protocol_options_t tcp) {
        nw_tcp_options_set_no_delay(tcp, true);
    });
    nw_parameters_set_reuse_local_address(params, true);
    NSString *port = [NSString stringWithFormat:@"%u", _port];
    nw_parameters_set_local_endpoint(params, nw_endpoint_create_host(PUListenHost, port.UTF8String));
    _listener = nw_listener_create(params);
    if (!_listener) {
        if (error) *error = PUError(PURunnerErrorFailed, [NSString stringWithFormat:@"cannot listen on %s:%@", PUListenHost, port]);
        return NO;
    }
    nw_listener_set_queue(_listener, _queue);
    __weak PUServer *weakSelf = self;
    nw_listener_set_new_connection_handler(_listener, ^(nw_connection_t connection) {
        [weakSelf accept:connection];
    });
    dispatch_semaphore_t ready = dispatch_semaphore_create(0);
    __block nw_error_t failure = nil;
    nw_listener_set_state_changed_handler(_listener, ^(nw_listener_state_t state, nw_error_t stateError) {
        if (state == nw_listener_state_ready || state == nw_listener_state_failed) {
            failure = stateError;
            dispatch_semaphore_signal(ready);
        }
    });
    nw_listener_start(_listener);
    dispatch_semaphore_wait(ready, DISPATCH_TIME_FOREVER);
    if (failure) {
        if (error) *error = PUError(PURunnerErrorFailed, [NSString stringWithFormat:@"listen on port %@ failed: %@", port, failure]);
        return NO;
    }
    NSLog(@"PhonedriverRunner listening on %s:%@", PUListenHost, port);
    return YES;
}

- (void)accept:(nw_connection_t)connection
{
    nw_connection_set_queue(connection, _queue);
    nw_connection_start(connection);
    [self readFrom:connection authorized:NO];
}

- (void)refuse:(nw_connection_t)connection request:(nullable NSDictionary *)request
{
    NSMutableDictionary *reply = [@{
        @"ok": @NO,
        @"error": @{@"code": @"unauthorized", @"message": @"this connection did not prove the runner's token"},
    } mutableCopy];
    if (request[@"id"]) reply[@"id"] = request[@"id"];
    NSData *frame = PUFrame(reply);
    dispatch_data_t data = dispatch_data_create(frame.bytes, frame.length, _queue, DISPATCH_DATA_DESTRUCTOR_DEFAULT);
    nw_connection_send(connection, data, NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true, ^(nw_error_t sendError) {
        nw_connection_cancel(connection);
    });
}

- (void)adopt:(nw_connection_t)connection
{
    if (_current && _current != connection) {
        nw_connection_cancel(_current);
    }
    _current = connection;
}

- (void)drop:(nw_connection_t)connection
{
    nw_connection_cancel(connection);
    if (_current == connection) {
        _current = nil;
    }
}

- (void)readFrom:(nw_connection_t)connection authorized:(BOOL)authorized
{
    nw_connection_receive(connection, 4, 4, ^(dispatch_data_t header, nw_content_context_t context, bool complete, nw_error_t error) {
        if (error || !header || dispatch_data_get_size(header) != 4) {
            [self drop:connection];
            return;
        }
        uint32_t length = CFSwapInt32BigToHost(*(const uint32_t *)((NSData *)header).bytes);
        if (length == 0 || length > (authorized ? PUMaxFrame : PUMaxFirstFrame)) {
            [self drop:connection];
            return;
        }
        nw_connection_receive(connection, length, length, ^(dispatch_data_t body, nw_content_context_t bodyContext, bool bodyComplete, nw_error_t bodyError) {
            if (bodyError || !body) {
                [self drop:connection];
                return;
            }
            if (authorized) {
                [self handle:(NSData *)body on:connection proof:nil];
                return;
            }
            NSDictionary *request = PUParse((NSData *)body, nil);
            NSString *proof = request ? PUAuthorize(request, self->_token, self->_seen) : nil;
            if (!proof) {
                [self refuse:connection request:request];
                return;
            }
            [self adopt:connection];
            [self handle:(NSData *)body on:connection proof:proof];
        });
    });
}

// XCTest UI APIs must run on the main thread, and some of them wait for replies that arrive on
// the main queue, so requests run as main run loop blocks rather than main queue blocks.
- (void)handle:(NSData *)body on:(nw_connection_t)connection proof:(nullable NSString *)proof
{
    PUHandler handler = _handler;
    CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopDefaultMode, ^{
        NSError *error = nil;
        NSDictionary *request = PUParse(body, &error);
        NSDictionary *reply = request
            ? handler(request)
            : @{@"ok": @NO, @"error": @{@"code": PUErrorCodeName(error), @"message": error.localizedDescription}};
        if (proof) {
            NSMutableDictionary *proven = [reply mutableCopy];
            proven[@"proof"] = proof;
            reply = proven;
        }
        NSData *frame = PUFrame(reply);
        dispatch_data_t data = dispatch_data_create(frame.bytes, frame.length, self->_queue, DISPATCH_DATA_DESTRUCTOR_DEFAULT);
        nw_connection_send(connection, data, NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true, ^(nw_error_t sendError) {
            if (sendError) {
                [self drop:connection];
            } else {
                [self readFrom:connection authorized:YES];
            }
        });
    });
    CFRunLoopWakeUp(CFRunLoopGetMain());
}

@end
