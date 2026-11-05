#import <CoreGraphics/CoreGraphics.h>
#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

extern NSErrorDomain const PURunnerErrorDomain;

typedef NS_ERROR_ENUM(PURunnerErrorDomain, PURunnerError) {
    PURunnerErrorBadRequest = 1,
    PURunnerErrorUnsupported = 2,
    PURunnerErrorFailed = 3,
};

NSError *PUError(PURunnerError code, NSString *message);

NSString *PUErrorCodeName(NSError *error);

NSData *PUFrame(NSDictionary *message);

NSDictionary *_Nullable PUParse(NSData *body, NSError **error);

NSNumber *PUPoint(CGFloat value);

NS_ASSUME_NONNULL_END
