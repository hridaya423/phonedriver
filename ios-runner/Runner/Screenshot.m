#import "Screenshot.h"

#import <XCTest/XCTest.h>

#import "Json.h"

static NSData *PULuma(CGImageRef image, size_t width, size_t height)
{
    NSMutableData *bytes = [NSMutableData dataWithLength:width * height];
    CGColorSpaceRef gray = CGColorSpaceCreateDeviceGray();
    CGContextRef context = CGBitmapContextCreate(bytes.mutableBytes, width, height, 8, width, gray, (CGBitmapInfo)kCGImageAlphaNone);
    CGColorSpaceRelease(gray);
    if (!context) return nil;
    CGContextSetInterpolationQuality(context, kCGInterpolationMedium);
    CGContextDrawImage(context, CGRectMake(0, 0, width, height), image);
    CGContextRelease(context);
    return bytes;
}

static UIImage *PUScaled(UIImage *image, size_t width, size_t height)
{
    UIGraphicsImageRendererFormat *format = [UIGraphicsImageRendererFormat preferredFormat];
    format.scale = 1;
    format.opaque = YES;
    UIGraphicsImageRenderer *renderer = [[UIGraphicsImageRenderer alloc] initWithSize:CGSizeMake(width, height) format:format];
    return [renderer imageWithActions:^(UIGraphicsImageRendererContext *context) {
        CGContextSetInterpolationQuality(context.CGContext, kCGInterpolationHigh);
        [image drawInRect:CGRectMake(0, 0, width, height)];
    }];
}

NSDictionary *PUScreenshot(NSString *format, NSInteger quality, double scale, NSError **error)
{
    BOOL png = [format isEqualToString:@"png"];
    BOOL luma = [format isEqualToString:@"luma"];
    if (!png && !luma && ![format isEqualToString:@"jpeg"]) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"format must be jpeg, png or luma");
        return nil;
    }
    if (quality < 0 || quality > 100) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"quality must be 0 to 100");
        return nil;
    }
    if (scale < 0 || scale > 4 || (luma && scale == 0)) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"scale must be above 0 and at most 4 (0 means native, except for luma)");
        return nil;
    }
    XCUIScreenshot *shot = XCUIScreen.mainScreen.screenshot;
    UIImage *image = shot.image;
    size_t width = (size_t)llround(scale > 0 ? image.size.width * scale : image.size.width * image.scale);
    size_t height = (size_t)llround(scale > 0 ? image.size.height * scale : image.size.height * image.scale);
    NSData *data = nil;
    if (luma) {
        data = PULuma(image.CGImage, width, height);
    } else if (scale > 0) {
        UIImage *scaled = PUScaled(image, width, height);
        data = png ? UIImagePNGRepresentation(scaled) : UIImageJPEGRepresentation(scaled, (CGFloat)quality / 100.0);
    } else {
        data = png ? shot.PNGRepresentation : UIImageJPEGRepresentation(image, (CGFloat)quality / 100.0);
    }
    if (!data) {
        if (error) *error = PUError(PURunnerErrorFailed, @"screenshot encoding failed");
        return nil;
    }
    return @{
        @"data_b64": [data base64EncodedStringWithOptions:0],
        @"w": @(width),
        @"h": @(height),
    };
}
