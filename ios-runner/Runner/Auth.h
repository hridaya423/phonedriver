#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// HMAC-SHA256 under `token` of `<side>:<nonce>`, as lowercase hex.
NSString *PUProof(NSString *token, NSString *side, NSString *nonce);

/// The runner's proof for the first frame of a connection, or nil when the frame is not a `hello` whose `proof` is
/// the host's proof of `token` over a nonce not in `seen` (decision D106). An accepted nonce joins `seen`, so a frame
/// read off the network does not open a second connection. An empty token accepts nothing.
NSString *_Nullable PUAuthorize(NSDictionary *request, NSString *token, NSMutableSet<NSString *> *seen);

NS_ASSUME_NONNULL_END
