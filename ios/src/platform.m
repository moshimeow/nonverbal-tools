#import <Foundation/Foundation.h>
#import <TargetConditionals.h>
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>
#else
#import <AppKit/AppKit.h>
#endif
#include <stdbool.h>

const char *speak_data_directory(void) {
    static NSString *directory;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSURL *base = [[NSFileManager defaultManager] URLForDirectory:NSApplicationSupportDirectory
            inDomain:NSUserDomainMask appropriateForURL:nil create:YES error:nil];
        directory = [[base URLByAppendingPathComponent:@"SpeakPocket" isDirectory:YES] path];
    });
    return directory.UTF8String;
}

bool speak_copy_text(const char *text) {
    @autoreleasepool {
        NSString *value = [[NSString alloc] initWithUTF8String:text];
        if (!value) return false;
        __block bool success = false;
        void (^copy)(void) = ^{
#if TARGET_OS_IOS
            [UIPasteboard generalPasteboard].string = value;
            success = true;
#else
            NSPasteboard *board = [NSPasteboard generalPasteboard];
            [board clearContents];
            success = [board setString:value forType:NSPasteboardTypeString];
#endif
        };
        if ([NSThread isMainThread]) copy();
        else dispatch_sync(dispatch_get_main_queue(), copy);
        return success;
    }
}
