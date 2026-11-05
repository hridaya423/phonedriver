#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

typedef NSDictionary *_Nonnull (^PUHandler)(NSDictionary *request);

@interface PUServer : NSObject
- (instancetype)initWithPort:(uint16_t)port handler:(PUHandler)handler;
- (BOOL)start:(NSError **)error;
@end

NS_ASSUME_NONNULL_END
