#import <Foundation/Foundation.h>

#import "Server.h"

int main(int argc, char **argv)
{
    @autoreleasepool {
        if (argc < 2) {
            fprintf(stderr, "usage: server_harness <port> [token]\n");
            return 2;
        }
        NSString *token = argc > 2 ? @(argv[2]) : @"";
        PUServer *server = [[PUServer alloc] initWithPort:(uint16_t)atoi(argv[1]) token:token handler:^NSDictionary *(NSDictionary *request) {
            return @{@"id": request[@"id"] ?: @0, @"ok": @YES, @"op": request[@"op"] ?: @"", @"version": @"0.2.0"};
        }];
        NSError *error = nil;
        if (![server start:&error]) {
            fprintf(stderr, "%s\n", error.localizedDescription.UTF8String);
            return 1;
        }
        printf("ready\n");
        fflush(stdout);
        CFRunLoopRun();
    }
    return 0;
}
