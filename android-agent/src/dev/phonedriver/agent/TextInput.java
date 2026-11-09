package dev.phonedriver.agent;

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
    private static final int FIELD_SEARCH_NODES = 3000;

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
        return quiet(c);
    }

    // SystemUI shows a clipboard preview over the bottom left of the screen for every new clip from Android 13, and
    // that window takes the next tap there. It skips the preview when a clip set by the shell carries this extra.
    private static ClipData quiet(ClipData c) {
        if (Build.VERSION.SDK_INT >= 33) {
            PersistableBundle extras = c.getDescription().getExtras();
            extras = extras == null ? new PersistableBundle() : new PersistableBundle(extras);
            extras.putBoolean("com.android.systemui.SUPPRESS_CLIPBOARD_OVERLAY", true);
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

    private static String current(AccessibilityNodeInfo field) throws Exception {
        if (field == null) {
            return textOf(Accessibility.focusedInput());
        }
        return field.refresh() ? textOf(field) : "";
    }

    // A WebView and Chrome's page container hold input focus for the page; the field itself is a focused, editable
    // node inside them.
    private static AccessibilityNodeInfo focusedEditableIn(AccessibilityNodeInfo focus) {
        if (focus.isEditable()) {
            return null;
        }
        java.util.ArrayDeque<AccessibilityNodeInfo> queue = new java.util.ArrayDeque<>();
        queue.add(focus);
        for (int seen = 0; !queue.isEmpty() && seen < FIELD_SEARCH_NODES; seen++) {
            AccessibilityNodeInfo n = queue.poll();
            if (n != focus && n.isFocused() && n.isEditable()) {
                return n;
            }
            for (int i = 0; i < n.getChildCount(); i++) {
                AccessibilityNodeInfo c = n.getChild(i);
                if (c != null) {
                    queue.add(c);
                }
            }
        }
        return null;
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
        AccessibilityNodeInfo focus = awaitFocusedInput();
        AccessibilityNodeInfo field = focus == null ? null : focusedEditableIn(focus);
        String before = textOf(field != null ? field : focus);
        ClipboardManager cm = clipboard();
        ClipData saved = cm.getClip();
        cm.setClip(clip(text, sensitive));
        try {
            Input.key(KeyEvent.KEYCODE_PASTE, "press");
            // spike A4: WebView and some Compose fields read the clipboard after the key returns,
            // so restoring at once pastes the old clip. Wait for the field to change. A WebView field that was
            // just loaded on the emulator read the clipboard more than 200 ms late in 4 of 10 pastes and took the
            // restored clip, so the limit is 1000 ms. In a page the focused input node is the WebView or Chrome's
            // page container, whose text never changes, so every web paste waited the whole 1000 ms; the page's own
            // focused field is watched instead.
            long deadline = SystemClock.uptimeMillis() + RESTORE_WAIT_MS;
            while (SystemClock.uptimeMillis() < deadline && current(field).equals(before)) {
                SystemClock.sleep(POLL_MS);
            }
        } finally {
            cm.setClip(quiet(saved != null ? saved : ClipData.newPlainText(null, "")));
        }
    }

    private static void set(String text) throws Exception {
        AccessibilityNodeInfo n = awaitFocusedInput();
        if (n == null) {
            throw new IllegalStateException("no focused input field");
        }
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (n.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) {
            return;
        }
        // Input focus in a web page is reported on the WebView itself, which has no set_text, and Contacts' phone field
        // (prefilled "+1") refuses it, so the field is selected with Ctrl+A and then deleted or pasted over.
        Input.selectAll();
        if (text.isEmpty()) {
            Input.key(KeyEvent.KEYCODE_DEL, "press");
        } else {
            paste(text, false);
        }
    }

    private TextInput() {}
}
