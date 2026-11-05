#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

BOOL PUGesture(NSArray *pointers, NSError **error);

@class XCSynthesizedEventRecord;

BOOL PUSynthesize(XCSynthesizedEventRecord *record, NSTimeInterval timeout, NSError **error);

NS_ASSUME_NONNULL_END
