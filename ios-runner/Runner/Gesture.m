#import "Gesture.h"

#import "Json.h"
#import "XCTestPrivate.h"

static const NSUInteger PUMaxFingers = 5;

static BOOL PUSpinUntil(BOOL (^done)(void), NSTimeInterval timeout)
{
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:timeout];
    while (!done()) {
        if (deadline.timeIntervalSinceNow < 0) return NO;
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.005, true);
    }
    return YES;
}

// Events are synthesized in the portrait frame whatever the orientation (observed on an iPad Pro 13-inch simulator
// in landscape: a tap at (519, 438) landed at (438, 513) in a 1376 x 1032 app, and the interfaceOrientation of the
// record made no difference). Points arrive in the screen as the app sees it, so they are rotated back.
static CGPoint PUPortraitPoint(CGPoint p)
{
    CGRect screen = [[XCUIApplication alloc] initWithBundleIdentifier:@"com.apple.springboard"].frame;
    CGFloat portraitWidth = MIN(screen.size.width, screen.size.height);
    CGFloat portraitHeight = MAX(screen.size.width, screen.size.height);
    switch (XCUIDevice.sharedDevice.orientation) {
        case UIDeviceOrientationLandscapeLeft: return CGPointMake(portraitWidth - p.y, p.x);
        case UIDeviceOrientationLandscapeRight: return CGPointMake(p.y, portraitHeight - p.x);
        case UIDeviceOrientationPortraitUpsideDown: return CGPointMake(portraitWidth - p.x, portraitHeight - p.y);
        default: return p;
    }
}

static XCPointerEventPath *PUPath(id pointer, double *end, NSError **error)
{
    NSArray *points = [pointer isKindOfClass:NSDictionary.class] ? pointer[@"path"] : nil;
    if (![points isKindOfClass:NSArray.class] || points.count == 0) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"each pointer needs a non-empty path");
        return nil;
    }
    XCPointerEventPath *path = nil;
    double last = -1;
    for (NSDictionary *p in points) {
        NSNumber *x = p[@"x"], *y = p[@"y"], *t = p[@"t_ms"];
        if (![x isKindOfClass:NSNumber.class] || ![y isKindOfClass:NSNumber.class] || ![t isKindOfClass:NSNumber.class]
            || t.doubleValue < last) {
            if (error) *error = PUError(PURunnerErrorBadRequest, @"path points need numeric x, y and non-decreasing t_ms");
            return nil;
        }
        CGPoint point = PUPortraitPoint(CGPointMake(x.doubleValue, y.doubleValue));
        double offset = t.doubleValue / 1000.0;
        if (!path) {
            path = [[XCPointerEventPath alloc] initForTouchAtPoint:point offset:offset];
        } else {
            [path moveToPoint:point atOffset:offset];
        }
        last = t.doubleValue;
    }
    [path liftUpAtOffset:last / 1000.0];
    *end = MAX(*end, last / 1000.0);
    return path;
}

BOOL PUGesture(NSArray *pointers, NSError **error)
{
    if (![pointers isKindOfClass:NSArray.class] || pointers.count == 0 || pointers.count > PUMaxFingers) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"pointers must hold 1 to 5 paths");
        return NO;
    }
    XCSynthesizedEventRecord *record = [[XCSynthesizedEventRecord alloc] initWithName:@"phonedriver gesture"
                                                                 interfaceOrientation:UIInterfaceOrientationPortrait];
    double end = 0;
    for (id pointer in pointers) {
        XCPointerEventPath *path = PUPath(pointer, &end, error);
        if (!path) return NO;
        [record addPointerEventPath:path];
    }
    return PUSynthesize(record, end + 10, error);
}

BOOL PUSynthesize(XCSynthesizedEventRecord *record, NSTimeInterval timeout, NSError **error)
{
    __block BOOL finished = NO;
    __block NSError *failure = nil;
    [XCUIDevice.sharedDevice.eventSynthesizer synthesizeEvent:record completion:^(BOOL success, NSError *synthError) {
        failure = success ? nil : (synthError ?: PUError(PURunnerErrorFailed, @"event synthesis failed"));
        finished = YES;
    }];
    if (!PUSpinUntil(^BOOL { return finished; }, timeout)) {
        if (error) *error = PUError(PURunnerErrorFailed, @"event synthesis did not complete");
        return NO;
    }
    if (failure && error) *error = failure;
    return failure == nil;
}
