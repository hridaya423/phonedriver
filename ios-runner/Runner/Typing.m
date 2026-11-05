#import "Typing.h"

#import "Gesture.h"
#import "Json.h"
#import "XCTestPrivate.h"

static const unsigned long long PUTypingSpeed = 60;

// XCUIElement typeText: writes the text into the test activity log (xcodebuild output and the
// .xcresult bundle). A synthesized text-input path with shouldRedact keeps secrets out of both.
BOOL PUType(NSString *text, NSError **error)
{
    if (![text isKindOfClass:NSString.class] || text.length == 0) {
        if (error) *error = PUError(PURunnerErrorBadRequest, @"text must be a non-empty string");
        return NO;
    }
    XCSynthesizedEventRecord *record = [[XCSynthesizedEventRecord alloc] initWithName:@"phone-use type"
                                                                 interfaceOrientation:UIInterfaceOrientationPortrait];
    XCPointerEventPath *path = [[XCPointerEventPath alloc] initForTextInput];
    [path typeText:text atOffset:0 typingSpeed:PUTypingSpeed shouldRedact:YES];
    [record addPointerEventPath:path];
    return PUSynthesize(record, (double)text.length / PUTypingSpeed + 10, error);
}
