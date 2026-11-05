#import "Snapshot.h"

#import "Json.h"
#import "XCTestPrivate.h"

#define PU_TYPE(name) [XCUIElementType##name] = @"XCUIElementType" #name

static NSString *const PUTypeNames[] = {
    PU_TYPE(Any), PU_TYPE(Other), PU_TYPE(Application), PU_TYPE(Group), PU_TYPE(Window), PU_TYPE(Sheet),
    PU_TYPE(Drawer), PU_TYPE(Alert), PU_TYPE(Dialog), PU_TYPE(Button), PU_TYPE(RadioButton),
    PU_TYPE(RadioGroup), PU_TYPE(CheckBox), PU_TYPE(DisclosureTriangle), PU_TYPE(PopUpButton),
    PU_TYPE(ComboBox), PU_TYPE(MenuButton), PU_TYPE(ToolbarButton), PU_TYPE(Popover), PU_TYPE(Keyboard),
    PU_TYPE(Key), PU_TYPE(NavigationBar), PU_TYPE(TabBar), PU_TYPE(TabGroup), PU_TYPE(Toolbar),
    PU_TYPE(StatusBar), PU_TYPE(Table), PU_TYPE(TableRow), PU_TYPE(TableColumn), PU_TYPE(Outline),
    PU_TYPE(OutlineRow), PU_TYPE(Browser), PU_TYPE(CollectionView), PU_TYPE(Slider), PU_TYPE(PageIndicator),
    PU_TYPE(ProgressIndicator), PU_TYPE(ActivityIndicator), PU_TYPE(SegmentedControl), PU_TYPE(Picker),
    PU_TYPE(PickerWheel), PU_TYPE(Switch), PU_TYPE(Toggle), PU_TYPE(Link), PU_TYPE(Image), PU_TYPE(Icon),
    PU_TYPE(SearchField), PU_TYPE(ScrollView), PU_TYPE(ScrollBar), PU_TYPE(StaticText), PU_TYPE(TextField),
    PU_TYPE(SecureTextField), PU_TYPE(DatePicker), PU_TYPE(TextView), PU_TYPE(Menu), PU_TYPE(MenuItem),
    PU_TYPE(MenuBar), PU_TYPE(MenuBarItem), PU_TYPE(Map), PU_TYPE(WebView), PU_TYPE(IncrementArrow),
    PU_TYPE(DecrementArrow), PU_TYPE(Timeline), PU_TYPE(RatingIndicator), PU_TYPE(ValueIndicator),
    PU_TYPE(SplitGroup), PU_TYPE(Splitter), PU_TYPE(RelevanceIndicator), PU_TYPE(ColorWell), PU_TYPE(HelpTag),
    PU_TYPE(Matte), PU_TYPE(DockItem), PU_TYPE(Ruler), PU_TYPE(RulerMarker), PU_TYPE(Grid),
    PU_TYPE(LevelIndicator), PU_TYPE(Cell), PU_TYPE(LayoutArea), PU_TYPE(LayoutItem), PU_TYPE(Handle),
    PU_TYPE(Stepper), PU_TYPE(Tab), PU_TYPE(TouchBar), PU_TYPE(StatusItem),
};

static NSString *PUTypeName(XCUIElementType type)
{
    NSUInteger count = sizeof PUTypeNames / sizeof PUTypeNames[0];
    NSString *name = (NSUInteger)type < count ? PUTypeNames[type] : nil;
    return name ?: @"XCUIElementTypeOther";
}

static BOOL PUIsTextEntry(XCUIElementType type)
{
    return type == XCUIElementTypeTextField || type == XCUIElementTypeSecureTextField
        || type == XCUIElementTypeSearchField || type == XCUIElementTypeTextView;
}

static BOOL PUIsCheckable(XCUIElementType type)
{
    return type == XCUIElementTypeSwitch || type == XCUIElementTypeToggle || type == XCUIElementTypeCheckBox;
}

static BOOL PUIsTappable(XCUIElementType type)
{
    switch (type) {
    case XCUIElementTypeButton:
    case XCUIElementTypeCell:
    case XCUIElementTypeLink:
    case XCUIElementTypeSwitch:
    case XCUIElementTypeToggle:
    case XCUIElementTypeCheckBox:
    case XCUIElementTypeRadioButton:
    case XCUIElementTypeTab:
    case XCUIElementTypeMenuItem:
    case XCUIElementTypeKey:
    case XCUIElementTypeIcon:
        return YES;
    default:
        return PUIsTextEntry(type);
    }
}

static BOOL PUIsScrollable(XCUIElementType type)
{
    return type == XCUIElementTypeScrollView || type == XCUIElementTypeTable
        || type == XCUIElementTypeCollectionView || type == XCUIElementTypeWebView;
}

static NSString *PUString(id value)
{
    if (value == nil || value == NSNull.null) return nil;
    NSString *s = [value isKindOfClass:NSString.class] ? value : [value description];
    return s.length ? s : nil;
}

static void PUFlatten(id<XCElementSnapshot> s, NSInteger parent, CGRect screen, NSMutableArray *out)
{
    NSInteger index = (NSInteger)out.count;
    XCUIElementType type = s.elementType;
    CGRect f = s.frame;
    NSMutableDictionary *n = [NSMutableDictionary dictionaryWithCapacity:8];
    n[@"p"] = @(parent);
    n[@"cls"] = PUTypeName(type);
    n[@"b"] = @[PUPoint(CGRectGetMinX(f)), PUPoint(CGRectGetMinY(f)), PUPoint(CGRectGetMaxX(f)), PUPoint(CGRectGetMaxY(f))];
    NSString *label = PUString(s.label);
    if (label) n[@"text"] = label;
    NSString *ident = PUString(s.identifier);
    if (ident) n[@"id"] = ident;
    NSString *value = PUString(s.value);
    if (value) n[@"sd"] = value;
    NSString *hint = PUString(s.placeholderValue);
    if (hint) n[@"hint"] = hint;

    NSMutableString *flags = [NSMutableString string];
    if (!CGRectIsEmpty(CGRectIntersection(f, screen))) [flags appendString:@"vV"];
    if (s.enabled) [flags appendString:@"e"];
    if (s.selected) [flags appendString:@"s"];
    if (s.hasKeyboardFocus || s.hasFocus) [flags appendString:@"F"];
    if (PUIsCheckable(type)) {
        [flags appendString:@"c"];
        if ([value isEqualToString:@"1"]) [flags appendString:@"C"];
    }
    if (PUIsTextEntry(type)) [flags appendString:@"E"];
    if (type == XCUIElementTypeSecureTextField) [flags appendString:@"P"];
    if (PUIsScrollable(type)) [flags appendString:@"S"];
    if (flags.length) n[@"f"] = flags;
    if (PUIsTappable(type)) n[@"a"] = @[@"tap"];
    [out addObject:n];

    for (id<XCElementSnapshot> child in s.children) {
        PUFlatten(child, index, screen, out);
    }
}

static XCAXClient_iOS *PUClient(void)
{
    return XCUIDevice.sharedDevice.accessibilityInterface;
}

// The AX attribute names behind label, value, identifier, frame, elementType, traits (enabled,
// selected), placeholderValue and hasFocus. Mapping those key paths through XCElementSnapshot
// adds window ids, automation type and base type too, which cost about 1.5 ms per snapshot and
// changed no node on any testbed tab (spike I4, docs/spikes/ios.md).
static NSArray<NSString *> *PUAttributes(void)
{
    return @[@"XC_kAXXCAttributeLabel", @"XC_kAXXCAttributeValue", @"XC_kAXXCAttributeIdentifier",
             @"XC_kAXXCAttributeFrame", @"XC_kAXXCAttributeElementType", @"XC_kAXXCAttributeTraits",
             @"XC_kAXXCAttributePlaceholderValue", @"XC_kAXXCAttributeHasNativeFocus"];
}

static id<XCAccessibilityElement> PUActiveElement(NSError **error)
{
    id<XCAccessibilityElement> element = PUClient().activeApplications.firstObject;
    if (!element && error) *error = PUError(PURunnerErrorFailed, @"no active application");
    return element;
}

static XCUIApplication *PUApplication(int pid, NSError **error)
{
    static NSMutableDictionary<NSNumber *, XCUIApplication *> *cache;
    if (!cache) cache = [NSMutableDictionary dictionary];
    XCUIApplication *app = cache[@(pid)];
    if (!app) {
        app = [PUClient().applicationProcessTracker monitoredApplicationWithProcessIdentifier:pid];
        if (app) cache[@(pid)] = app;
    }
    if (!app && error) *error = PUError(PURunnerErrorFailed, [NSString stringWithFormat:@"no application for pid %d", pid]);
    return app;
}

NSDictionary *PUSnapshot(NSInteger maxDepth, NSError **error)
{
    id<XCAccessibilityElement> element = PUActiveElement(error);
    if (!element) return nil;
    int pid = element.processIdentifier;
    XCUIApplication *app = PUApplication(pid, error);
    if (!app) return nil;

    NSMutableDictionary *params = [PUClient().defaultParameters mutableCopy];
    params[@"maxDepth"] = @(maxDepth);
    id result = [PUClient() requestSnapshotForElement:element attributes:PUAttributes() parameters:params error:error];
    if (!result) return nil;
    id<XCElementSnapshot> root = [result valueForKey:@"_rootElementSnapshot"];
    if (!root) {
        if (error) *error = PUError(PURunnerErrorFailed, @"snapshot has no root element");
        return nil;
    }

    NSMutableArray *nodes = [NSMutableArray arrayWithCapacity:256];
    PUFlatten(root, -1, root.frame, nodes);
    return @{@"app": @{@"bundle_id": app.bundleID ?: @"", @"pid": @(pid)}, @"nodes": nodes};
}
