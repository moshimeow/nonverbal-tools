-- Run with the desktop prototype open: osascript ios/scripts/ui-smoke.applescript
-- Uses Accessibility permission, speaks short phrases, and edits only this app.
on waitUntilReady(root)
 tell application "System Events"
  repeat 100 times
   delay 0.2
   set message to value of static text 1 of group 3 of root
   if message is "Ready" then return
   if message is not in {"Making speech…", "Loading voice model…", "Speaking…"} then error "Unexpected speech status: " & message
  end repeat
 end tell
 error "Speech did not finish within 20 seconds"
end waitUntilReady

tell application "System Events"
 tell process "speak-pocket"
  set frontmost to true
  set size of window 1 to {430, 900}
  set root to group 1 of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
  click checkbox "Emma British English" of group "Voice" of root
  click text area 1 of root
  keystroke "a" using command down
  keystroke "Hello! This is a test of the Rust app."
  delay 0.2
  if value of text area 1 of root is not "Hello! This is a test of the Rust app." then error "Typing did not update the message"
  if not enabled of button "Speak" of root then error "Speak did not enable after typing"
  click button "Speak" of root
 end tell
end tell
my waitUntilReady(root)
tell application "System Events"
 tell process "speak-pocket"
  -- Stop during playback.
  click button "Repeat" of root
  repeat 100 times
   delay 0.1
   if value of static text 1 of group 3 of root is "Speaking…" then exit repeat
  end repeat
  if value of static text 1 of group 3 of root is not "Speaking…" then error "Repeat did not begin playback"
  click button "Stop" of root
  delay 0.2
  if value of static text 1 of group 3 of root is not "Stopped" then error "Stop failed during playback"
  -- Stop during synthesis; stale worker events must not overwrite Stopped.
  click button "Repeat" of root
  delay 0.1
  click button "Stop" of root
  delay 5
  if value of static text 1 of group 3 of root is not "Stopped" then error "A canceled request overwrote the Stop state"
  -- Exercise the speed input through its native accessibility action.
  set oldSpeed to value of slider "Speed" of root
  perform action "AXIncrement" of slider "Speed" of root
  delay 0.2
  if (value of slider "Speed" of root as real) ≤ (oldSpeed as real) then error "Speed slider did not update"
  perform action "AXDecrement" of slider "Speed" of root
  click checkbox "晓晓 普通话 · Mandarin" of group "Voice" of root
  click button "请稍等，我正在打字。" of root
 end tell
end tell
my waitUntilReady(root)
tell application "System Events"
 tell process "speak-pocket"
  if value of text area 1 of root is not "请稍等，我正在打字。" then error "Mandarin phrase did not populate text"
  -- Refocus after synthesis: the user or build tools may have activated Xcode.
  set frontmost to true
  delay 0.2
  click text area 1 of root
  keystroke "a" using command down
  delay 0.1
  key code 51
  repeat 20 times
   delay 0.1
   if not enabled of button "Speak" of root then exit repeat
  end repeat
  if value of text area 1 of root is not "" then error "Clear did not reach the focused text field"
  if enabled of button "Speak" of root then error "Speak should be disabled for empty input"
  click checkbox "Emma British English" of group "Voice" of root
 end tell
end tell
return "PASS: typed speech, repeat, stop during playback, stop during generation, speed, Mandarin, empty input"
