#import <CoreLocation/CoreLocation.h>
#import <Foundation/Foundation.h>
typedef void (*Reply)(void *, bool, double, double, double, double);
@interface SpeakLocation : NSObject <CLLocationManagerDelegate>
@property CLLocationManager *manager;
@property NSMutableDictionary<NSUUID *, void (^)(CLLocation *)> *requests;
@end
@implementation SpeakLocation
- (instancetype)init {
    if ((self = [super init])) {
        _requests = [NSMutableDictionary dictionary];
        _manager = [CLLocationManager new];
        _manager.delegate = self;
        _manager.desiredAccuracy = kCLLocationAccuracyHundredMeters;
    }
    return self;
}
- (void)finish:(CLLocation *)location {
    NSArray *callbacks = self.requests.allValues;
    [self.requests removeAllObjects];
    for (void (^callback)(CLLocation *) in callbacks) callback(location);
}
- (void)request {
    switch (self.manager.authorizationStatus) {
        case kCLAuthorizationStatusNotDetermined: [self.manager requestWhenInUseAuthorization]; break;
        case kCLAuthorizationStatusDenied:
        case kCLAuthorizationStatusRestricted: [self finish:nil]; break;
        default: [self.manager requestLocation]; break;
    }
}
- (void)locationManagerDidChangeAuthorization:(CLLocationManager *)manager {
    if (self.requests.count) [self request];
}
- (void)locationManager:(CLLocationManager *)manager didUpdateLocations:(NSArray<CLLocation *> *)locations {
    CLLocation *location = locations.lastObject;
    if (location.horizontalAccuracy >= 0 && fabs(location.timestamp.timeIntervalSinceNow) < 60) [self finish:location];
}
- (void)locationManager:(CLLocationManager *)manager didFailWithError:(NSError *)error { [self finish:nil]; }
@end
void speak_location(void *context, Reply reply) {
    dispatch_async(dispatch_get_main_queue(), ^{
        static SpeakLocation *service;
        if (!service) service = [SpeakLocation new];
        NSUUID *key = [NSUUID UUID];
        service.requests[key] = ^(CLLocation *location) {
            reply(context, location != nil, location.coordinate.latitude, location.coordinate.longitude,
                  location.horizontalAccuracy, location.timestamp.timeIntervalSince1970);
        };
        [service request];
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            void (^callback)(CLLocation *) = service.requests[key];
            [service.requests removeObjectForKey:key];
            if (callback) callback(nil);
        });
    });
}
void speak_timestamp(double seconds, char *buffer, size_t capacity) {
    @autoreleasepool {
        NSDateFormatter *formatter = [NSDateFormatter new];
        formatter.dateStyle = NSDateFormatterMediumStyle;
        formatter.timeStyle = NSDateFormatterMediumStyle;
        NSString *text = [formatter stringFromDate:[NSDate dateWithTimeIntervalSince1970:seconds]];
        snprintf(buffer, capacity, "%s", text.UTF8String);
    }
}
