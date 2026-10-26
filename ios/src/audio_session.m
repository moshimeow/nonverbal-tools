#import <AVFAudio/AVFAudio.h>
#include <stdbool.h>

bool speak_configure_audio(void) {
    @autoreleasepool {
        AVAudioSession *session = [AVAudioSession sharedInstance];
        NSError *error = nil;
        if (![session setCategory:AVAudioSessionCategoryPlayback error:&error]) return false;
        return [session setActive:YES error:&error];
    }
}
