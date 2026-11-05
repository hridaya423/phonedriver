#import "Server.h"

#import <Network/Network.h>

#import "Json.h"

static const uint32_t PUMaxFrame = 16 * 1024 * 1024;

@implementation PUServer {
    uint16_t _port;
    PUHandler _handler;
    dispatch_queue_t _queue;
    nw_listener_t _listener;
    nw_connection_t _current;
}

- (instancetype)initWithPort:(uint16_t)port handler:(PUHandler)handler
{
    if ((self = [super init])) {
        _port = port;
        _handler = [handler copy];
        _queue = dispatch_queue_create("dev.phoneuse.runner.server", DISPATCH_QUEUE_SERIAL);
    }
    return self;
}

- (BOOL)start:(NSError **)error
{
    nw_parameters_t params = nw_parameters_create_secure_tcp(NW_PARAMETERS_DISABLE_PROTOCOL, ^(nw_protocol_options_t tcp) {
        nw_tcp_options_set_no_delay(tcp, true);
    });
    nw_parameters_set_reuse_local_address(params, true);
    NSString *port = [NSString stringWithFormat:@"%u", _port];
    nw_parameters_set_local_endpoint(params, nw_endpoint_create_host("127.0.0.1", port.UTF8String));
    _listener = nw_listener_create(params);
    if (!_listener) {
        if (error) *error = PUError(PURunnerErrorFailed, [NSString stringWithFormat:@"cannot listen on 127.0.0.1:%@", port]);
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
    NSLog(@"PhoneUseRunner listening on 127.0.0.1:%@", port);
    return YES;
}

- (void)accept:(nw_connection_t)connection
{
    if (_current) {
        nw_connection_cancel(_current);
    }
    _current = connection;
    nw_connection_set_queue(connection, _queue);
    nw_connection_start(connection);
    [self readFrom:connection];
}

- (void)drop:(nw_connection_t)connection
{
    nw_connection_cancel(connection);
    if (_current == connection) {
        _current = nil;
    }
}

- (void)readFrom:(nw_connection_t)connection
{
    nw_connection_receive(connection, 4, 4, ^(dispatch_data_t header, nw_content_context_t context, bool complete, nw_error_t error) {
        if (error || !header || dispatch_data_get_size(header) != 4) {
            [self drop:connection];
            return;
        }
        uint32_t length = CFSwapInt32BigToHost(*(const uint32_t *)((NSData *)header).bytes);
        if (length == 0 || length > PUMaxFrame) {
            [self drop:connection];
            return;
        }
        nw_connection_receive(connection, length, length, ^(dispatch_data_t body, nw_content_context_t bodyContext, bool bodyComplete, nw_error_t bodyError) {
            if (bodyError || !body) {
                [self drop:connection];
                return;
            }
            [self handle:(NSData *)body on:connection];
        });
    });
}

// XCTest UI APIs must run on the main thread, and some of them wait for replies that arrive on
// the main queue, so requests run as main run loop blocks rather than main queue blocks.
- (void)handle:(NSData *)body on:(nw_connection_t)connection
{
    PUHandler handler = _handler;
    CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopDefaultMode, ^{
        NSError *error = nil;
        NSDictionary *request = PUParse(body, &error);
        NSDictionary *reply = request
            ? handler(request)
            : @{@"ok": @NO, @"error": @{@"code": PUErrorCodeName(error), @"message": error.localizedDescription}};
        NSData *frame = PUFrame(reply);
        dispatch_data_t data = dispatch_data_create(frame.bytes, frame.length, self->_queue, DISPATCH_DATA_DESTRUCTOR_DEFAULT);
        nw_connection_send(connection, data, NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true, ^(nw_error_t sendError) {
            if (sendError) {
                [self drop:connection];
            } else {
                [self readFrom:connection];
            }
        });
    });
    CFRunLoopWakeUp(CFRunLoopGetMain());
}

@end
