#import <XCTest/XCTest.h>
#import <notify.h>
#import <sys/utsname.h>

#import "Buttons.h"
#import "Gesture.h"
#import "Json.h"
#import "Screenshot.h"
#import "Server.h"
#import "Snapshot.h"
#import "Typing.h"
#import "XCTestPrivate.h"

static NSString *const PURunnerVersion = @"0.2.0";
static const uint16_t PUDefaultPort = 27753;
static const NSInteger PUDefaultMaxDepth = 30;

@interface PhonedriverRunnerTests : XCTestCase
@property (nonatomic, nullable) NSString *issue;
@end

static NSString *PUModel(void)
{
#if TARGET_OS_SIMULATOR
    return NSProcessInfo.processInfo.environment[@"SIMULATOR_MODEL_IDENTIFIER"] ?: UIDevice.currentDevice.model;
#else
    struct utsname info;
    uname(&info);
    return @(info.machine);
#endif
}

// The runner's own process runs letterboxed (it has no launch screen), so UIScreen reports
// 320 x 480 there; SpringBoard's frame is the real screen in points.
// SpringBoard posts its lock state under this name. The tree of a locked iPhone is the lock screen's, which reads
// like any other screen, so a tap sent to a locked iPhone 13 Pro Max answered "no element matches" (2026-11-13).
static BOOL PULocked(void)
{
    static int token;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ notify_register_check("com.apple.springboard.lockstate", &token); });
    uint64_t state = 0;
    notify_get_state(token, &state);
    return state != 0;
}

static NSDictionary *PUHello(void)
{
    CGRect bounds = [[XCUIApplication alloc] initWithBundleIdentifier:@"com.apple.springboard"].frame;
    NSDictionary *env = NSProcessInfo.processInfo.environment;
    NSMutableDictionary *hello = [@{
        @"version": PURunnerVersion,
        @"ios": UIDevice.currentDevice.systemVersion,
        @"model": PUModel(),
        @"screen": @{@"w": @(bounds.size.width), @"h": @(bounds.size.height), @"scale": @(UIScreen.mainScreen.scale)},
    } mutableCopy];
    NSString *udid = env[@"SIMULATOR_UDID"] ?: env[@"PHONEDRIVER_UDID"];
    if (udid) hello[@"udid"] = udid;
    if (env[@"PHONEDRIVER_BUILD"]) hello[@"build"] = env[@"PHONEDRIVER_BUILD"];
    hello[@"locked"] = @(PULocked());
    return hello;
}

static NSString *PUStringField(NSDictionary *request, NSString *key, NSError **error)
{
    id value = request[key];
    if (![value isKindOfClass:NSString.class]) {
        if (error) *error = PUError(PURunnerErrorBadRequest, [NSString stringWithFormat:@"%@ must be a string", key]);
        return nil;
    }
    return value;
}

// The names of the months and of the halves of the day, each form at the same place for every month: full, full
// standing alone, short, short standing alone, first in the language the device is set to and then in English.
// An app draws its pickers in its own language, which is English when it has no translation for the device's:
// on a Simulator set to ja_JP the testbed, then English only, showed `Jan 15, 2026` (iOS 26.5).
static NSDictionary *PUCalendarNames(void)
{
    NSMutableArray<NSMutableArray *> *months = [NSMutableArray array];
    for (NSUInteger month = 0; month < 12; month++) [months addObject:[NSMutableArray array]];
    NSMutableArray *am = [NSMutableArray array];
    NSMutableArray *pm = [NSMutableArray array];
    for (NSLocale *locale in @[NSLocale.autoupdatingCurrentLocale, [NSLocale localeWithLocaleIdentifier:@"en"]]) {
        NSDateFormatter *formatter = [[NSDateFormatter alloc] init];
        formatter.locale = locale;
        formatter.calendar = [[NSCalendar alloc] initWithCalendarIdentifier:NSCalendarIdentifierGregorian];
        for (NSUInteger month = 0; month < 12; month++) {
            [months[month] addObjectsFromArray:@[
                formatter.monthSymbols[month], formatter.standaloneMonthSymbols[month],
                formatter.shortMonthSymbols[month], formatter.shortStandaloneMonthSymbols[month],
            ]];
        }
        [am addObject:formatter.AMSymbol ?: @""];
        [pm addObject:formatter.PMSymbol ?: @""];
    }
    return @{
        @"locale": NSLocale.autoupdatingCurrentLocale.localeIdentifier,
        @"months": months,
        @"am": am,
        @"pm": pm,
    };
}

static NSDictionary *PUPerform(NSDictionary *request, NSError **error)
{
    NSString *op = request[@"op"];
    if ([op isEqualToString:@"hello"]) {
        return PUHello();
    }
    if ([op isEqualToString:@"snapshot"]) {
        NSNumber *depth = request[@"max_depth"];
        return PUSnapshot([depth isKindOfClass:NSNumber.class] ? depth.integerValue : PUDefaultMaxDepth, error);
    }
    if ([op isEqualToString:@"gesture"]) {
        return PUGesture(request[@"pointers"], error) ? @{} : nil;
    }
    if ([op isEqualToString:@"banner"]) {
        return @{@"covered": @(PUBannerAt(CGPointMake([request[@"x"] doubleValue], [request[@"y"] doubleValue])))};
    }
    if ([op isEqualToString:@"screenshot"]) {
        NSString *format = request[@"format"] ?: @"jpeg";
        NSNumber *quality = request[@"quality"] ?: @80;
        NSNumber *scale = request[@"scale"] ?: @0;
        if (![format isKindOfClass:NSString.class] || ![quality isKindOfClass:NSNumber.class]
            || ![scale isKindOfClass:NSNumber.class]) {
            if (error) *error = PUError(PURunnerErrorBadRequest, @"format must be a string, quality and scale numbers");
            return nil;
        }
        // The host polls screenshots all the time, so the lock state rides on them at no extra request.
        NSMutableDictionary *shot = [PUScreenshot(format, quality.integerValue, scale.doubleValue, error) mutableCopy];
        shot[@"locked"] = @(PULocked());
        return shot;
    }
    if ([op isEqualToString:@"type"]) {
        NSString *text = PUStringField(request, @"text", error);
        return text && PUType(text, [request[@"speed"] unsignedIntegerValue], error) ? @{} : nil;
    }
    if ([op isEqualToString:@"key"]) {
        NSString *key = PUStringField(request, @"key", error);
        return key && PUKey(key, request[@"modifiers"] ?: @[], error) ? @{} : nil;
    }
    if ([op isEqualToString:@"calendar_names"]) {
        return PUCalendarNames();
    }
    if ([op isEqualToString:@"awake"]) {
        return PUAwake(error) ? @{} : nil;
    }
    if ([op isEqualToString:@"button"]) {
        NSString *name = PUStringField(request, @"name", error);
        return name && PUButton(name, error) ? @{} : nil;
    }
    if ([op isEqualToString:@"activate"]) {
        NSString *bundleId = PUStringField(request, @"bundle_id", error);
        if (!bundleId) return nil;
        [[[XCUIApplication alloc] initWithBundleIdentifier:bundleId] activate];
        return @{};
    }
    if ([op isEqualToString:@"open_url"]) {
        NSString *string = PUStringField(request, @"url", error);
        NSURL *url = string ? [NSURL URLWithString:string] : nil;
        if (!url) {
            if (error && string) *error = PUError(PURunnerErrorBadRequest, @"url is not a valid URL");
            return nil;
        }
        [XCUIDevice.sharedDevice.system openURL:url];
        return @{};
    }
    if (error) *error = PUError(PURunnerErrorBadRequest, [NSString stringWithFormat:@"unknown op %@", op]);
    return nil;
}

@implementation PhonedriverRunnerTests

// XCTest reports UI API failures (for example typing with no focused field) as test issues and,
// with continueAfterFailure off, aborts the test. Capturing them turns each into an error reply.
- (void)recordIssue:(XCTIssue *)issue
{
    self.issue = [issue.compactDescription componentsSeparatedByString:@"\n"].firstObject;
}

- (NSDictionary *)reply:(NSDictionary *)request
{
    self.issue = nil;
    NSError *error = nil;
    NSDictionary *result = nil;
    @try {
        result = PUPerform(request, &error);
    } @catch (NSException *exception) {
        error = PUError(PURunnerErrorFailed, [NSString stringWithFormat:@"%@: %@", exception.name, exception.reason]);
        result = nil;
    }
    if (result && self.issue) {
        error = PUError(PURunnerErrorFailed, self.issue);
        result = nil;
    }
    NSMutableDictionary *reply = [NSMutableDictionary dictionaryWithDictionary:result ?: @{}];
    if (request[@"id"]) reply[@"id"] = request[@"id"];
    reply[@"ok"] = result ? @YES : @NO;
    if (!result) {
        reply[@"error"] = @{@"code": PUErrorCodeName(error), @"message": error.localizedDescription ?: @"failed"};
    }
    return reply;
}

// On a Simulator just booted on a CI machine, SpringBoard's first answer to a query took 40.7 s (GitHub's macOS
// runner, 2026-11-17: the first snapshot's look for alerts ran from 2.8 s to 43.5 s after the runner listened), and
// the runner answers nothing else meanwhile. So it asks once before it listens, and is ready only when it can answer.
- (void)testServe
{
    self.continueAfterFailure = YES;
    (void)[[XCUIApplication alloc] initWithBundleIdentifier:@"com.apple.springboard"].alerts.count;
    NSDictionary *env = NSProcessInfo.processInfo.environment;
    NSString *portString = env[@"PHONEDRIVER_PORT"];
    uint16_t port = portString.length ? (uint16_t)portString.integerValue : PUDefaultPort;
    NSString *token = env[@"PHONEDRIVER_TOKEN"] ?: @"";
    if (token.length == 0) {
        NSLog(@"PhonedriverRunner was started without PHONEDRIVER_TOKEN and refuses every connection");
    }
    __weak PhonedriverRunnerTests *weakSelf = self;
    PUServer *server = [[PUServer alloc] initWithPort:port token:token handler:^NSDictionary *(NSDictionary *request) {
        return [weakSelf reply:request];
    }];
    NSError *error = nil;
    if (![server start:&error]) {
        XCTFail(@"%@", error.localizedDescription);
        return;
    }
    while (YES) {
        @autoreleasepool {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0e10, false);
        }
    }
}

@end
