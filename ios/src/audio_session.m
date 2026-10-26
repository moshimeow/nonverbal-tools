#import <AVFAudio/AVFAudio.h>
#import <Foundation/Foundation.h>
#include <stdbool.h>
#include <stdint.h>
#include <math.h>

// FFI status values: starting, playing, complete, canceled, failed.
typedef NS_ENUM(NSInteger, SpeakStatus) {
    SpeakStarting = 0, SpeakPlaying = 1, SpeakComplete = 2,
    SpeakCanceled = 3, SpeakFailed = 4
};

@interface SpeakTicket : NSObject
@property(atomic) SpeakStatus status;
@property(atomic) BOOL canceled;
@property(atomic, copy) NSString *error;
@property(nonatomic) NSData *samples;
@property(nonatomic) uint32_t sampleRate;
@property(nonatomic) NSTimeInterval deadline;
@end
@implementation SpeakTicket
@end

@interface SpeakAudio : NSObject
@property(nonatomic) NSThread *thread;
@property(nonatomic) AVAudioEngine *engine;
@property(nonatomic) AVAudioPlayerNode *player;
@property(nonatomic) SpeakTicket *current;
@property(nonatomic) BOOL sessionActive;
@property(atomic) BOOL queuePending;
@property(atomic, copy) NSString *sessionError;
@property(nonatomic) NSUInteger releaseAttempts;
@property(nonatomic) NSUInteger releaseEpoch;
- (void)enqueue:(dispatch_block_t)block;
- (void)start:(SpeakTicket *)ticket;
- (void)cancel:(SpeakTicket *)ticket;
- (void)releaseIfIdle;
@end

@implementation SpeakAudio
- (instancetype)init {
    if ((self = [super init])) {
        dispatch_semaphore_t ready = dispatch_semaphore_create(0);
        _thread = [[NSThread alloc] initWithBlock:^{
            @autoreleasepool {
                // Engine callbacks and the watchdog have a live run loop, without
                // occupying the UI thread or the Rust queue coordinator.
                NSRunLoop *loop = NSRunLoop.currentRunLoop;
                [loop addPort:[NSMachPort port] forMode:NSDefaultRunLoopMode];
                [NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *timer) {
                    (void)timer;
                    SpeakTicket *ticket = self.current;
                    if (!ticket) return;
                    if (ticket.canceled) [self cancel:ticket];
                    else if (ticket.status == SpeakPlaying && NSProcessInfo.processInfo.systemUptime > ticket.deadline)
                        [self fail:ticket message:@"Speech playback stalled"];
                }];
                dispatch_semaphore_signal(ready);
                [loop run];
            }
        }];
        _thread.name = @"Speak Pocket audio";
        [_thread start];
        dispatch_semaphore_wait(ready, DISPATCH_TIME_FOREVER);
        NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
        // The notification center retains these process-lifetime observers.
        [center addObserverForName:AVAudioSessionInterruptionNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
            if ([note.userInfo[AVAudioSessionInterruptionTypeKey] unsignedIntegerValue] != AVAudioSessionInterruptionTypeBegan) return;
            [self enqueue:^{
                self.sessionActive = NO; // iOS has already deactivated our session.
                self.sessionError = nil;
                if (self.current) [self fail:self.current message:@"Speech was interrupted"];
                else [self stopEngine];
            }];
        }];
        [center addObserverForName:AVAudioSessionRouteChangeNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
            (void)note;
            [self enqueue:^{
                if (self.current && ![self hasSpeakerRoute])
                    [self fail:self.current message:@"Audio output changed during speech"];
            }];
        }];
        [center addObserverForName:AVAudioSessionMediaServicesWereResetNotification object:nil queue:nil usingBlock:^(NSNotification *note) {
            (void)note;
            [self enqueue:^{
                self.sessionActive = NO;
                self.sessionError = nil;
                if (self.current) [self fail:self.current message:@"Audio services restarted during speech"];
                else [self stopEngine];
            }];
        }];
    }
    return self;
}
- (void)runBlock:(dispatch_block_t)block {
    @autoreleasepool { block(); }
}
- (void)enqueue:(dispatch_block_t)block {
    [self performSelector:@selector(runBlock:) onThread:self.thread withObject:[block copy] waitUntilDone:NO];
}
- (BOOL)hasSpeakerRoute {
    NSArray<AVAudioSessionPortDescription *> *ports = AVAudioSession.sharedInstance.currentRoute.outputs;
    return ports.count == 1 && [ports.firstObject.portType isEqualToString:AVAudioSessionPortBuiltInSpeaker];
}
- (BOOL)activate:(NSError **)error {
    AVAudioSession *session = AVAudioSession.sharedInstance;
    // Deliberately non-mixing: pause other audio, speak, then release it. No
    // Bluetooth hands-free profile or multiroute channel map is needed.
    if (![session.category isEqualToString:AVAudioSessionCategoryPlayAndRecord] ||
        ![session.mode isEqualToString:AVAudioSessionModeDefault] ||
        session.categoryOptions != AVAudioSessionCategoryOptionDefaultToSpeaker) {
        if (![session setCategory:AVAudioSessionCategoryPlayAndRecord mode:AVAudioSessionModeDefault
                         options:AVAudioSessionCategoryOptionDefaultToSpeaker error:error]) return NO;
    }
    if (!self.sessionActive) {
        if (![session setActive:YES error:error]) return NO;
        self.sessionActive = YES;
    }
    if (![self hasSpeakerRoute] && ![session overrideOutputAudioPort:AVAudioSessionPortOverrideSpeaker error:error]) return NO;
    if (![self hasSpeakerRoute]) {
        *error = [NSError errorWithDomain:@"SpeakPocketAudio" code:1
            userInfo:@{NSLocalizedDescriptionKey: @"The iPhone speaker is unavailable"}];
        return NO;
    }
    self.releaseAttempts = 0;
    self.releaseEpoch += 1; // Cancel any old idle-release retries.
    self.sessionError = nil;
    return YES;
}
- (void)stopEngine {
    [self.player stop];
    [self.engine stop];
    self.player = nil;
    self.engine = nil;
}
- (void)fail:(SpeakTicket *)ticket message:(NSString *)message {
    ticket.error = message;
    if (self.current == ticket) {
        self.current = nil;
        [self stopEngine];
    }
    ticket.status = ticket.canceled ? SpeakCanceled : SpeakFailed;
    [self releaseIfIdle];
}
- (void)start:(SpeakTicket *)ticket {
    if (ticket.canceled) { ticket.status = SpeakCanceled; [self releaseIfIdle]; return; }
    self.current = ticket;
    NSError *error = nil;
    if (![self activate:&error]) { [self fail:ticket message:error.localizedDescription ?: @"Cannot activate speaker audio"]; return; }
    // Stop may arrive while iOS is activating audio. It sets an atomic flag,
    // so a canceled item never starts speaking after activation returns.
    if (ticket.canceled) { [self cancel:ticket]; return; }
    AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:ticket.sampleRate channels:1];
    AVAudioFrameCount count = (AVAudioFrameCount)(ticket.samples.length / sizeof(float));
    AVAudioPCMBuffer *buffer = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:count];
    if (!buffer) { [self fail:ticket message:@"Cannot allocate speech buffer"]; return; }
    buffer.frameLength = count;
    const float *samples = ticket.samples.bytes;
    for (AVAudioFrameCount i = 0; i < count; i++) {
        if (!isfinite(samples[i])) { [self fail:ticket message:@"Invalid speech sample"]; return; }
        buffer.floatChannelData[0][i] = samples[i];
    }
    ticket.samples = nil;
    if (self.engine && [self.player outputFormatForBus:0].sampleRate != ticket.sampleRate) [self stopEngine];
    if (!self.engine) {
        self.engine = [AVAudioEngine new];
        self.player = [AVAudioPlayerNode new];
        [self.engine attachNode:self.player];
        [self.engine connect:self.player to:self.engine.mainMixerNode format:format];
    }
    if (!self.engine.running && ![self.engine startAndReturnError:&error]) {
        [self fail:ticket message:error.localizedDescription ?: @"Cannot start speaker playback"]; return;
    }
    if (ticket.canceled) { [self cancel:ticket]; return; }
    ticket.deadline = NSProcessInfo.processInfo.systemUptime + (double)count / ticket.sampleRate + 3.0;
    [self.player scheduleBuffer:buffer completionCallbackType:AVAudioPlayerNodeCompletionDataPlayedBack completionHandler:^(AVAudioPlayerNodeCompletionCallbackType type) {
        (void)type;
        [self enqueue:^{
            if (self.current != ticket) return;
            self.current = nil;
            [self.player stop];
            ticket.status = ticket.canceled ? SpeakCanceled : SpeakComplete;
            [self releaseIfIdle];
        }];
    }];
    [self.player play];
    ticket.status = SpeakPlaying;
}
- (void)cancel:(SpeakTicket *)ticket {
    if (self.current == ticket) {
        self.current = nil; // Ignore late completion from the canceled buffer.
        [self.player stop];
    }
    ticket.status = SpeakCanceled;
    [self releaseIfIdle];
}
- (void)releaseIfIdle {
    if (self.queuePending || self.current) return;
    [self stopEngine];
    if (!self.sessionActive) return;
    NSError *error = nil;
    AVAudioSession *session = AVAudioSession.sharedInstance;
    [session overrideOutputAudioPort:AVAudioSessionPortOverrideNone error:NULL];
    if ([session setActive:NO withOptions:AVAudioSessionSetActiveOptionNotifyOthersOnDeactivation error:&error]) {
        self.sessionActive = NO; // Only mark inactive after confirmed success.
        self.sessionError = nil;
        self.releaseAttempts = 0;
        return;
    }
    self.sessionError = [NSString stringWithFormat:@"Could not release audio; music may remain paused: %@", error.localizedDescription];
    // Audio I/O may take a moment to stop. Retry without blocking either UI or
    // queue processing; retain the active state and expose failure if it persists.
    if (++self.releaseAttempts <= 3) {
        NSUInteger epoch = self.releaseEpoch;
        double delay = 0.25 * self.releaseAttempts;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)), dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            [self enqueue:^{ if (epoch == self.releaseEpoch) [self releaseIfIdle]; }];
        });
    }
}
@end

static SpeakAudio *controller(void) {
    static SpeakAudio *audio;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ audio = [SpeakAudio new]; });
    return audio;
}
static void copy_message(NSString *message, char *buffer, size_t capacity) {
    if (capacity) snprintf(buffer, capacity, "%s", (message ?: @"").UTF8String);
}
void *speak_play_audio(const float *samples, size_t count, uint32_t rate) {
    @autoreleasepool {
        SpeakTicket *ticket = [SpeakTicket new];
        if (!samples || !count || count > UINT32_MAX || count > SIZE_MAX / sizeof(float) || !rate) {
            ticket.error = @"Invalid speech buffer"; ticket.status = SpeakFailed;
        } else {
            ticket.samples = [NSData dataWithBytes:samples length:count * sizeof(float)];
            ticket.sampleRate = rate;
            ticket.status = SpeakStarting;
            SpeakAudio *audio = controller();
            [audio enqueue:^{ [audio start:ticket]; }];
    }
    return (__bridge_retained void *)ticket;
    }
}
int speak_audio_status(void *handle) {
    @autoreleasepool {
        return (int)((__bridge SpeakTicket *)handle).status;
    }
}
void speak_audio_error(void *handle, char *buffer, size_t capacity) {
    @autoreleasepool {
        copy_message(((__bridge SpeakTicket *)handle).error, buffer, capacity);
    }
}
void speak_stop_audio(void *handle) {
    @autoreleasepool {
        SpeakTicket *ticket = (__bridge SpeakTicket *)handle;
        if (ticket.status == SpeakComplete || ticket.status == SpeakFailed || ticket.status == SpeakCanceled) return;
        ticket.canceled = YES;
        SpeakAudio *audio = controller();
        [audio enqueue:^{ [audio cancel:ticket]; }];
    }
}
void speak_release_audio(void *handle) {
    @autoreleasepool {
        speak_stop_audio(handle);
        (void)CFBridgingRelease(handle);
    }
}
void speak_audio_queue_pending(bool pending) {
    @autoreleasepool {
        SpeakAudio *audio = controller();
        audio.queuePending = pending;
        if (!pending) [audio enqueue:^{ [audio releaseIfIdle]; }];
    }
}
void speak_audio_session_error(char *buffer, size_t capacity) {
    @autoreleasepool {
        copy_message(controller().sessionError, buffer, capacity);
    }
}
