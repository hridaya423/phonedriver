#import "Auth.h"

#import <CommonCrypto/CommonHMAC.h>
#import <string.h>

static NSData *PUMac(NSString *token, NSString *side, NSString *nonce)
{
    NSData *key = [token dataUsingEncoding:NSUTF8StringEncoding];
    NSData *message = [[NSString stringWithFormat:@"%@:%@", side, nonce] dataUsingEncoding:NSUTF8StringEncoding];
    NSMutableData *mac = [NSMutableData dataWithLength:CC_SHA256_DIGEST_LENGTH];
    CCHmac(kCCHmacAlgSHA256, key.bytes, key.length, message.bytes, message.length, mac.mutableBytes);
    return mac;
}

static NSString *PUHex(NSData *data)
{
    const uint8_t *bytes = data.bytes;
    NSMutableString *hex = [NSMutableString stringWithCapacity:data.length * 2];
    for (NSUInteger i = 0; i < data.length; i++) {
        [hex appendFormat:@"%02x", bytes[i]];
    }
    return hex;
}

static NSData *_Nullable PUUnhex(NSString *hex)
{
    if (hex.length % 2 != 0) return nil;
    NSMutableData *data = [NSMutableData dataWithLength:hex.length / 2];
    uint8_t *out = data.mutableBytes;
    for (NSUInteger i = 0; i < hex.length; i += 2) {
        unsigned value = 0;
        for (NSUInteger j = i; j < i + 2; j++) {
            unichar c = [hex characterAtIndex:j];
            unsigned digit;
            if (c >= '0' && c <= '9') digit = c - '0';
            else if (c >= 'a' && c <= 'f') digit = c - 'a' + 10;
            else if (c >= 'A' && c <= 'F') digit = c - 'A' + 10;
            else return nil;
            value = value * 16 + digit;
        }
        out[i / 2] = (uint8_t)value;
    }
    return data;
}

NSString *PUProof(NSString *token, NSString *side, NSString *nonce)
{
    return PUHex(PUMac(token, side, nonce));
}

NSString *PUAuthorize(NSDictionary *request, NSString *token, NSMutableSet<NSString *> *seen)
{
    NSString *nonce = request[@"nonce"];
    NSString *proof = request[@"proof"];
    if (token.length == 0 || ![request[@"op"] isEqual:@"hello"] || ![nonce isKindOfClass:NSString.class]
        || ![proof isKindOfClass:NSString.class] || nonce.length < 32 || nonce.length > 128
        || [seen containsObject:nonce]) {
        return nil;
    }
    NSData *expected = PUMac(token, @"host", nonce);
    NSData *given = PUUnhex(proof);
    if (given.length != expected.length || timingsafe_bcmp(given.bytes, expected.bytes, expected.length) != 0) {
        return nil;
    }
    [seen addObject:nonce];
    return PUProof(token, @"runner", nonce);
}
