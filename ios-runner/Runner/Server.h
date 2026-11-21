#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

typedef NSDictionary *_Nonnull (^PUHandler)(NSDictionary *request);

@interface PUServer : NSObject
/// Serves `handler` on `port` to one connection at a time; a connection's first frame must prove `token`.
- (instancetype)initWithPort:(uint16_t)port token:(NSString *)token handler:(PUHandler)handler;
- (BOOL)start:(NSError **)error;
@end

NS_ASSUME_NONNULL_END
