#import "Screenshot.h"

#import <XCTest/XCTest.h>

#import "Json.h"

NSDictionary *PUScreenshot(NSString *format, NSInteger quality, NSError **error)
{
    BOOL png = [format isEqualToString:@"png"];
    if (!png && ![format isEqualToString:@"jpeg"]) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"format must be jpeg or png");
        return nil;
    }
    if (quality < 0 || quality > 100) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"quality must be 0 to 100");
        return nil;
    }
    XCUIScreenshot *shot = XCUIScreen.mainScreen.screenshot;
    UIImage *image = shot.image;
    NSData *data = png ? shot.PNGRepresentation : UIImageJPEGRepresentation(image, (CGFloat)quality / 100.0);
    if (!data) {
        if (error) *error = PUError(PURunnerErrorFailed, @"screenshot encoding failed");
        return nil;
    }
    return @{
        @"data_b64": [data base64EncodedStringWithOptions:0],
        @"w": @(llround(image.size.width * image.scale)),
        @"h": @(llround(image.size.height * image.scale)),
    };
}
