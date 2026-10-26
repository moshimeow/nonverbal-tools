#import <AVFAudio/AVFAudio.h>
#include <stdbool.h>

// All session changes run on one serial queue. Notification handlers enqueue work
// so a route change caused by our own override cannot re-enter configuration.
static dispatch_queue_t routing_queue;
static bool interrupted = false;

static bool is_speaker(AVAudioSession *session) {
    NSArray<AVAudioSessionPortDescription *> *outputs = session.currentRoute.outputs;
    return outputs.count == 1 && [outputs.firstObject.portType isEqualToString:AVAudioSessionPortBuiltInSpeaker];
}

static bool enforce_speaker(bool activate) {
    AVAudioSession *session = [AVAudioSession sharedInstance];
    NSError *error = nil;
    // Bluetooth/AirPlay options are deliberately absent. No input AudioUnit is
    // created: PlayAndRecord is required by Apple's speaker override API only.
    if (![session.category isEqualToString:AVAudioSessionCategoryPlayAndRecord] ||
        session.categoryOptions != AVAudioSessionCategoryOptionDefaultToSpeaker) {
        if (![session setCategory:AVAudioSessionCategoryPlayAndRecord
                            mode:AVAudioSessionModeDefault
                         options:AVAudioSessionCategoryOptionDefaultToSpeaker error:&error]) {
            NSLog(@"Speak Pocket: cannot configure speaker: %@", error);
            return false;
        }
    }
    if (activate && ![session setActive:YES error:&error]) {
        NSLog(@"Speak Pocket: cannot activate audio: %@", error);
        return false;
    }
    if (!is_speaker(session) &&
        ![session overrideOutputAudioPort:AVAudioSessionPortOverrideSpeaker error:&error]) {
        NSLog(@"Speak Pocket: speaker override failed: %@", error);
        return false;
    }
    return is_speaker(session);
}

bool speak_configure_audio(void) {
    @autoreleasepool {
        static dispatch_once_t once;
        dispatch_once(&once, ^{
            routing_queue = dispatch_queue_create("com.moshi.speakpocket.audio-route", DISPATCH_QUEUE_SERIAL);
            NSNotificationCenter *center = [NSNotificationCenter defaultCenter];
            // Retained for the process lifetime, like the audio output worker.
            static id route_observer;
            static id interruption_observer;
            static id reset_observer;
            route_observer = [center addObserverForName:AVAudioSessionRouteChangeNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
                dispatch_async(routing_queue, ^{
                    if (!interrupted && !is_speaker([AVAudioSession sharedInstance])) {
                        enforce_speaker(false);
                    }
                });
            }];
            interruption_observer = [center addObserverForName:AVAudioSessionInterruptionNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
                dispatch_async(routing_queue, ^{
                    interrupted = [note.userInfo[AVAudioSessionInterruptionTypeKey] unsignedIntegerValue] == AVAudioSessionInterruptionTypeBegan;
                    NSUInteger options = [note.userInfo[AVAudioSessionInterruptionOptionKey] unsignedIntegerValue];
                    if (!interrupted && (options & AVAudioSessionInterruptionOptionShouldResume)) enforce_speaker(true);
                });
            }];
            reset_observer = [center addObserverForName:AVAudioSessionMediaServicesWereResetNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
                dispatch_async(routing_queue, ^{ if (!interrupted) enforce_speaker(true); });
            }];
        });
        __block bool success = false;
        dispatch_sync(routing_queue, ^{ if (!interrupted) success = enforce_speaker(true); });
        return success;
    }
}
