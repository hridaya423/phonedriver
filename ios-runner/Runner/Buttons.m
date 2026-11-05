#import "Buttons.h"

#import "Json.h"
#import "XCTestPrivate.h"

BOOL PUButton(NSString *name, NSError **error)
{
    XCUIDevice *device = XCUIDevice.sharedDevice;
    if ([name isEqualToString:@"home"]) {
        [device pressButton:XCUIDeviceButtonHome];
    } else if ([name isEqualToString:@"lock"]) {
        [device pressLockButton];
    } else if ([name isEqualToString:@"volume_up"] || [name isEqualToString:@"volume_down"]) {
#if TARGET_OS_SIMULATOR
        if (error) *error = PUError(PURunnerErrorUnsupported, @"the Simulator has no volume buttons");
        return NO;
#else
        [device pressButton:[name isEqualToString:@"volume_up"] ? XCUIDeviceButtonVolumeUp : XCUIDeviceButtonVolumeDown];
#endif
    } else {
        if (error) *error = PUError(PURunnerErrorBadRequest, [NSString stringWithFormat:@"unknown button %@", name]);
        return NO;
    }
    return YES;
}
