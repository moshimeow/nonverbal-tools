-- Run with the desktop app open. Speaks samples and edits only its composer.
-- Refuse to type unless both the app and its text field have keyboard focus.
on sendText(message)
 tell application "System Events"
  tell process "speak-pocket"
   set frontmost to true
   delay 0.15
   set root to group 1 of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
   click text area "Message" of root
   if value of text area "Message" of root is not "" then error "Composer contains a draft; refusing to overwrite it"
   if not frontmost then error "App lost focus; refusing to type"
   if not (value of attribute "AXFocused" of text area "Message" of root) then error "Composer is not focused"
   keystroke message
   delay 0.2
   if value of text area "Message" of root is not message then error "Typed text mismatch"
   click button "▶ Play" of root
   delay 0.25
   if value of text area "Message" of root is not "" then error "Send did not clear composer"
  end tell
 end tell
end sendText
on waitEmpty()
 tell application "System Events"
  tell process "speak-pocket"
   set root to group 1 of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
   repeat 300 times
    if (count of groups of group "Speech queue" of root) is 0 then return
    delay 0.2
   end repeat
  end tell
 end tell
 error "Queue did not drain"
end waitEmpty
my waitEmpty()
tell application "System Events"
 tell process "speak-pocket"
  set size of window 1 to {430, 850}
  set root to group 1 of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
  if exists group "Speech settings" of root then click button "Language and voice settings" of group 1 of root
 end tell
end tell
my sendText("Henlo! This blorptastic floof wants to zorbulate.")
my sendText("Second queued sentence.")
my sendText("Remove this queued sentence.")
tell application "System Events"
 tell process "speak-pocket"
  if (count of groups of group "Speech queue" of root) < 2 then error "Sending replaced an existing message"
  set lastRow to last group of group "Speech queue" of root
  click button "Remove this message" of lastRow
 end tell
end tell
my waitEmpty()
tell application "System Events"
 tell process "speak-pocket"
  if (count of buttons of group "Recently spoken" of root) is not 2 then error "Expected two recent messages"
  click button "Say again: Second queued sentence." of group "Recently spoken" of root
  delay 0.2
  if (count of groups of group "Speech queue" of root) is not 1 then error "Replay did not enqueue"
  click button "Language and voice settings" of group 1 of root
  if not (exists pop up button "Language" of group "Speech settings" of root) then error "Missing language menu"
  if not (exists pop up button "Voice" of group "Speech settings" of root) then error "Missing voice menu"
  set oldSpeed to value of slider "Speed" of group "Speech settings" of root
  perform action "AXIncrement" of slider "Speed" of group "Speech settings" of root
  delay 0.1
  if (value of slider "Speed" of group "Speech settings" of root as real) ≤ (oldSpeed as real) then error "Speed did not change"
  perform action "AXDecrement" of slider "Speed" of group "Speech settings" of root
  click button "Language and voice settings" of group 1 of root
 end tell
end tell
my waitEmpty()
return "PASS: typing, clear-on-send, queue append/removal, two recent items, replay, settings, speed"
