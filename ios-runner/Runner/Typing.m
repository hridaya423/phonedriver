#import "Typing.h"

#import <XCTest/XCTest.h>

#import "Gesture.h"
#import "Json.h"
#import "XCTestPrivate.h"

static const unsigned long long PUTypingSpeed = 60;

// XCUIElement typeText: writes the text into the test activity log (xcodebuild output and the
// .xcresult bundle). A synthesized text-input path with shouldRedact keeps secrets out of both.
BOOL PUType(NSString *text, NSUInteger speed, NSError **error)
{
    if (![text isKindOfClass:NSString.class] || text.length == 0) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"text must be a non-empty string");
        return NO;
    }
    XCSynthesizedEventRecord *record = [[XCSynthesizedEventRecord alloc] initWithName:@"phonedriver type"
                                                                 interfaceOrientation:UIInterfaceOrientationPortrait];
    XCPointerEventPath *path = [[XCPointerEventPath alloc] initForTextInput];
    [path typeText:text atOffset:0 typingSpeed:speed ?: PUTypingSpeed shouldRedact:YES];
    [record addPointerEventPath:path];
    return PUSynthesize(record, (double)text.length / (speed ?: PUTypingSpeed) + 10, error);
}

// Shift down and up with no key: the idle timer counts it as use, and nothing else reacts. Observed 2026-11-13 on
// an iPhone 13 Pro Max with Auto-Lock at 30 s: one every 10 s held the screen awake with Settings in front, the
// on-screen keyboard stayed up, and text typed after it kept its case.
BOOL PUAwake(NSError **error)
{
    XCSynthesizedEventRecord *record = [[XCSynthesizedEventRecord alloc] initWithName:@"phonedriver awake"
                                                                 interfaceOrientation:UIInterfaceOrientationPortrait];
    XCPointerEventPath *path = [[XCPointerEventPath alloc] initForTextInput];
    [path setModifiers:XCUIKeyModifierShift mergeWithCurrentModifierFlags:NO atOffset:0];
    [path setModifiers:0 mergeWithCurrentModifierFlags:NO atOffset:0.05];
    [record addPointerEventPath:path];
    return PUSynthesize(record, 10, error);
}

BOOL PUKey(NSString *key, NSArray *modifiers, NSError **error)
{
    if (![key isKindOfClass:NSString.class] || key.length == 0 || ![modifiers isKindOfClass:NSArray.class]) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"key must be a non-empty string and modifiers a list");
        return NO;
    }
    NSDictionary<NSString *, NSNumber *> *names = @{
        @"command": @(XCUIKeyModifierCommand),
        @"shift": @(XCUIKeyModifierShift),
        @"option": @(XCUIKeyModifierOption),
        @"control": @(XCUIKeyModifierControl),
    };
    unsigned long long flags = 0;
    for (id name in modifiers) {
        NSNumber *flag = [name isKindOfClass:NSString.class] ? names[name] : nil;
        if (!flag) {
            if (error) *error = PUError(PURunnerErrorBadRequest, [NSString stringWithFormat:@"unknown modifier %@", name]);
            return NO;
        }
        flags |= flag.unsignedLongLongValue;
    }
    XCSynthesizedEventRecord *record = [[XCSynthesizedEventRecord alloc] initWithName:@"phonedriver key"
                                                                 interfaceOrientation:UIInterfaceOrientationPortrait];
    XCPointerEventPath *path = [[XCPointerEventPath alloc] initForTextInput];
    [path typeKey:key modifiers:flags atOffset:0];
    [record addPointerEventPath:path];
    return PUSynthesize(record, 10, error);
}
