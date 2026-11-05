// Declarations of private XCTest API, trimmed to what PhoneUseRunner calls.
// Derived from WebDriverAgent's PrivateHeaders/XCTest (BSD-3, see NOTICE):
// XCSynthesizedEventRecord.h, XCPointerEventPath.h, XCUIDevice.h, XCAXClient_iOS.h,
// XCUIApplicationProcessTracker-Protocol.h, XCUIApplication.h, and
// WebDriverAgentLib/Routing/FBXCElementSnapshot.h.
// Private API drifts between Xcode releases; this matches Xcode 27.0 (27A266a).

#import <XCTest/XCTest.h>
#import <UIKit/UIKit.h>

NS_ASSUME_NONNULL_BEGIN

@interface XCPointerEventPath : NSObject
- (id)initForTouchAtPoint:(CGPoint)point offset:(double)offset;
- (id)initForTextInput;
- (void)typeText:(NSString *)text atOffset:(double)offset typingSpeed:(unsigned long long)speed shouldRedact:(BOOL)redact;
- (void)moveToPoint:(CGPoint)point atOffset:(double)offset;
- (void)liftUpAtOffset:(double)offset;
@end

@interface XCSynthesizedEventRecord : NSObject
- (id)initWithName:(NSString *)name interfaceOrientation:(long long)orientation;
- (void)addPointerEventPath:(XCPointerEventPath *)path;
@end

@protocol XCUIEventSynthesizing <NSObject>
- (void)synthesizeEvent:(XCSynthesizedEventRecord *)event
             completion:(void (^)(BOOL success, NSError *_Nullable error))completion;
@end

@protocol XCUIApplicationProcessTracker <NSObject>
- (nullable XCUIApplication *)monitoredApplicationWithProcessIdentifier:(int)pid;
@end

@protocol XCAccessibilityElement <NSObject>
@property (readonly) int processIdentifier;
@end

@interface XCAXClient_iOS : NSObject
@property (readonly) id<XCUIApplicationProcessTracker> applicationProcessTracker;
- (NSArray<id<XCAccessibilityElement>> *)activeApplications;
- (NSDictionary *)defaultParameters;
- (nullable id)requestSnapshotForElement:(id<XCAccessibilityElement>)element
                              attributes:(NSArray *)attributes
                              parameters:(NSDictionary *)parameters
                                   error:(NSError **)error;
@end

@interface XCUIDevice (PhoneUsePrivate)
- (XCAXClient_iOS *)accessibilityInterface;
- (id<XCUIEventSynthesizing>)eventSynthesizer;
- (void)pressLockButton;
@end

@interface XCUIApplication (PhoneUsePrivate)
@property (readonly) NSString *bundleID;
@end

@protocol XCElementSnapshot <XCUIElementAttributes>
@property (copy) NSArray<id<XCElementSnapshot>> *children;
@property BOOL hasKeyboardFocus;
@end

NS_ASSUME_NONNULL_END
