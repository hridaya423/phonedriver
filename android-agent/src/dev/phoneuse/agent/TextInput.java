package dev.phoneuse.agent;

import android.content.ClipData;
import android.os.Build;
import android.os.Bundle;
import android.os.PersistableBundle;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import com.genymobile.scrcpy.wrappers.ClipboardManager;
import com.genymobile.scrcpy.wrappers.ServiceManager;

final class TextInput {
    private static final long RESTORE_WAIT_MS = 1000;
    private static final long POLL_MS = 10;
    private static final long FOCUS_WAIT_MS = 1500;

    private static ClipboardManager clipboard() {
        ClipboardManager cm = ServiceManager.getClipboardManager();
        if (cm == null) {
            throw new IllegalStateException("this device has no clipboard service");
        }
        return cm;
    }

    private static ClipData clip(String text, boolean sensitive) {
        ClipData c = ClipData.newPlainText(null, text);
        if (sensitive && Build.VERSION.SDK_INT >= 33) {
            PersistableBundle extras = new PersistableBundle();
            extras.putBoolean("android.content.extra.IS_SENSITIVE", true);
            c.getDescription().setExtras(extras);
        }
        return c;
    }

    static CharSequence clipboardText() {
        return clipboard().getText();
    }

    static void setClipboardText(String text, boolean sensitive) {
        clipboard().setClip(clip(text, sensitive));
    }

    static void run(String mode, String text, boolean sensitive) throws Exception {
        switch (mode) {
            case "paste":
                paste(text, sensitive);
                break;
            case "set":
                set(text);
                break;
            case "keys":
                Input.keys(text);
                break;
            default:
                throw new IllegalArgumentException("unknown text mode " + mode);
        }
    }

    private static String textOf(AccessibilityNodeInfo n) {
        CharSequence t = n == null ? null : n.getText();
        return t == null ? "" : t.toString();
    }

    // The field a tap just opened (Settings search on API 37) takes focus a few hundred ms after it appears in the
    // tree, and a paste before that goes nowhere while the reply says ok.
    private static AccessibilityNodeInfo awaitFocusedInput() throws Exception {
        long deadline = SystemClock.uptimeMillis() + FOCUS_WAIT_MS;
        AccessibilityNodeInfo n = Accessibility.focusedInput();
        while (n == null && SystemClock.uptimeMillis() < deadline) {
            SystemClock.sleep(POLL_MS);
            n = Accessibility.focusedInput();
        }
        return n;
    }

    private static void paste(String text, boolean sensitive) throws Exception {
        String before = textOf(awaitFocusedInput());
        ClipboardManager cm = clipboard();
        ClipData saved = cm.getClip();
        cm.setClip(clip(text, sensitive));
        try {
            Input.key(KeyEvent.KEYCODE_PASTE, "press");
            // spike A4: WebView and some Compose fields read the clipboard after the key returns,
            // so restoring at once pastes the old clip. Wait for the field to change. A WebView field that was
            // just loaded on the emulator read the clipboard more than 200 ms late in 4 of 10 pastes and took the
            // restored clip, so the limit is 1000 ms; the field text of a WebView is not exposed in time to end
            // the wait early.
            long deadline = SystemClock.uptimeMillis() + RESTORE_WAIT_MS;
            while (SystemClock.uptimeMillis() < deadline && textOf(Accessibility.focusedInput()).equals(before)) {
                SystemClock.sleep(POLL_MS);
            }
        } finally {
            cm.setClip(saved != null ? saved : ClipData.newPlainText(null, ""));
        }
    }

    private static void set(String text) throws Exception {
        AccessibilityNodeInfo n = awaitFocusedInput();
        if (n == null) {
            throw new IllegalStateException("no focused input field");
        }
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (!n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) {
            throw new IllegalStateException("the field refused set_text");
        }
    }

    private TextInput() {}
}
