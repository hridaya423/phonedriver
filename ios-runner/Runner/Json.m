#import "Json.h"

NSErrorDomain const PURunnerErrorDomain = @"dev.phoneuse.runner";

NSError *PUError(PURunnerError code, NSString *message)
{
    return [NSError errorWithDomain:PURunnerErrorDomain
                               code:code
                           userInfo:@{NSLocalizedDescriptionKey: message}];
}

NSString *PUErrorCodeName(NSError *error)
{
    if (![error.domain isEqualToString:PURunnerErrorDomain]) {
        return @"failed";
    }
    switch ((PURunnerError)error.code) {
    case PURunnerErrorBadRequest: return @"bad_request";
    case PURunnerErrorUnsupported: return @"unsupported";
    case PURunnerErrorFailed: return @"failed";
    }
    return @"failed";
}

NSData *PUFrame(NSDictionary *message)
{
    NSData *json = [NSJSONSerialization dataWithJSONObject:message options:0 error:nil];
    uint32_t length = CFSwapInt32HostToBig((uint32_t)json.length);
    NSMutableData *frame = [NSMutableData dataWithBytes:&length length:sizeof length];
    [frame appendData:json];
    return frame;
}

NSDictionary *PUParse(NSData *body, NSError **error)
{
    id object = [NSJSONSerialization JSONObjectWithData:body options:0 error:nil];
    if (![object isKindOfClass:NSDictionary.class]) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"frame is not a JSON object");
        return nil;
    }
    return object;
}

NSNumber *PUPoint(CGFloat value)
{
    long long hundredths = llround(value * 100);
    return [NSDecimalNumber decimalNumberWithMantissa:(unsigned long long)llabs(hundredths)
                                             exponent:-2
                                           isNegative:hundredths < 0];
}
