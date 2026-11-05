#import <XCTest/XCTest.h>
#import <QuartzCore/QuartzCore.h>

@interface NSObject (SpikePrivate)
- (id)accessibilityInterface;
- (NSArray *)activeApplications;
- (NSDictionary *)defaultParameters;
- (id)requestSnapshotForElement:(id)element attributes:(id)attributes parameters:(id)parameters error:(NSError **)error;
- (int)processIdentifier;
+ (id)axAttributesForElementSnapshotKeyPaths:(id)keyPaths isMacOS:(BOOL)isMacOS;
+ (NSArray *)sanitizedElementSnapshotHierarchyAttributesForAttributes:(NSArray *)attributes isMacOS:(BOOL)isMacOS;
@end

@interface SpikeTests : XCTestCase
@end

static void walk(id snap, NSInteger parent, NSInteger depth, NSMutableArray *out, NSInteger *maxDepth, CGRect screen)
{
  NSInteger me = (NSInteger)out.count;
  *maxDepth = MAX(*maxDepth, depth);
  CGRect f = [[snap valueForKey:@"frame"] CGRectValue];
  NSMutableDictionary *n = [NSMutableDictionary dictionary];
  n[@"p"] = @(parent);
  n[@"cls"] = [snap valueForKey:@"elementType"];
  id label = [snap valueForKey:@"label"];
  if ([label length]) n[@"text"] = label;
  id ident = [snap valueForKey:@"identifier"];
  if ([ident length]) n[@"id"] = ident;
  id value = [snap valueForKey:@"value"];
  if (value && value != NSNull.null) n[@"sd"] = [value description];
  n[@"b"] = @[@(f.origin.x), @(f.origin.y), @(CGRectGetMaxX(f)), @(CGRectGetMaxY(f))];
  n[@"f"] = CGRectIntersectsRect(f, screen) ? @"vVe" : @"e";
  [out addObject:n];
  for (id child in [snap valueForKey:@"children"]) {
    walk(child, me, depth + 1, out, maxDepth, screen);
  }
}

static double pct(NSArray<NSNumber *> *xs, double p)
{
  NSArray *s = [xs sortedArrayUsingSelector:@selector(compare:)];
  NSUInteger i = (NSUInteger)llround(p / 100.0 * (double)(s.count - 1));
  return [s[i] doubleValue];
}

@implementation SpikeTests

- (void)testTimeSnapshots
{
  NSInteger runs = 20;
  id ax = [XCUIDevice.sharedDevice accessibilityInterface];
  Class snapClass = NSClassFromString(@"XCElementSnapshot");
  NSArray *reducedKeys = @[@"label", @"value", @"identifier", @"frame", @"elementType", @"enabled", @"selected", @"hasFocus", @"placeholderValue"];
  NSArray *standardKeys = [snapClass sanitizedElementSnapshotHierarchyAttributesForAttributes:nil isMacOS:NO];
  NSArray *reduced = [[snapClass axAttributesForElementSnapshotKeyPaths:reducedKeys isMacOS:NO] allObjects];
  NSArray *standard = standardKeys;
  NSLog(@"SPIKE axClient=%@ standardKeys=%@ reducedAX=%@ standardAX=%@ defaults=%@",
        NSStringFromClass([ax class]), standardKeys, reduced, standard, [ax defaultParameters]);

  NSArray *screens = @[
    @[@"testbed list", @"dev.phoneuse.testbed", @[@"-tab", @"list"]],
    @[@"testbed taps", @"dev.phoneuse.testbed", @[@"-tab", @"taps"]],
    @[@"settings", @"com.apple.Preferences", @[]],
  ];
  for (NSArray *s in screens) {
    XCUIApplication *app = [[XCUIApplication alloc] initWithBundleIdentifier:s[1]];
    app.launchArguments = s[2];
    [app launch];
    [NSThread sleepForTimeInterval:3];
    id element = [[ax activeApplications] firstObject];
    CGRect screen = app.frame;

    NSDictionary *(^variant)(NSString *, id (^)(void), BOOL) = ^NSDictionary *(NSString *name, id (^take)(void), BOOL flatten) {
      NSMutableArray *times = [NSMutableArray array];
      NSInteger nodes = 0, maxDepth = 0;
      for (NSInteger i = -2; i < runs; i++) {
        @autoreleasepool {
          double t0 = CACurrentMediaTime();
          id root = take();
          NSMutableArray *out = [NSMutableArray array];
          NSInteger md = 0;
          if (flatten) {
            walk(root, -1, 1, out, &md, screen);
            NSData *json = [NSJSONSerialization dataWithJSONObject:@{@"nodes": out} options:0 error:nil];
            (void)json;
          }
          double ms = (CACurrentMediaTime() - t0) * 1000;
          if (i >= 0) [times addObject:@(ms)];
          if (!flatten) walk(root, -1, 1, out, &md, screen);
          nodes = (NSInteger)out.count;
          maxDepth = md;
        }
      }
      return @{@"screen": s[0], @"variant": name, @"runs": @(runs), @"p50_ms": @(pct(times, 50)),
               @"p95_ms": @(pct(times, 95)), @"min_ms": @(pct(times, 0)), @"nodes": @(nodes), @"tree_depth": @(maxDepth)};
    };

    id (^axSnapshot)(id, int) = ^id(id attrs, int depth) {
      NSMutableDictionary *params = [[ax defaultParameters] mutableCopy];
      params[@"maxDepth"] = @(depth);
      NSError *error = nil;
      id result = [ax requestSnapshotForElement:element attributes:attrs parameters:params error:&error];
      XCTAssertNotNil(result, @"%@", error);
      @try {
        id root = [result valueForKey:@"_rootElementSnapshot"];
        return root ?: result;
      } @catch (NSException *e) {
        return result;
      }
    };

    NSMutableArray *rows = [NSMutableArray array];
    [rows addObject:variant(@"public XCUIApplication snapshotWithError", ^id { NSError *e; return [app snapshotWithError:&e]; }, NO)];
    [rows addObject:variant(@"ax standard attrs maxDepth 30", ^id { return axSnapshot(standard, 30); }, NO)];
    [rows addObject:variant(@"ax reduced attrs maxDepth 30", ^id { return axSnapshot(reduced, 30); }, NO)];
    [rows addObject:variant(@"ax reduced attrs maxDepth 30 + flatten + JSON", ^id { return axSnapshot(reduced, 30); }, YES)];
    for (NSDictionary *row in rows) {
      NSData *json = [NSJSONSerialization dataWithJSONObject:row options:NSJSONWritingSortedKeys error:nil];
      NSLog(@"SPIKE %@", [[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding]);
    }
  }
}

@end
