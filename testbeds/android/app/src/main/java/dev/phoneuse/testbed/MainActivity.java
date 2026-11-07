package dev.phoneuse.testbed;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;

public final class MainActivity extends androidx.activity.ComponentActivity {
    private static final String[] TABS = {
        "taps", "text", "list", "gestures", "busy", "secure", "dialogs", "web", "settle", "wait", "compose", "checkout", "signup", "shop", "relay", "relaylog", "drain"
    };

    private FrameLayout content;
    private String current;
    private View panel;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setOnApplyWindowInsetsListener(
                (v, insets) -> {
                    v.setPadding(
                            insets.getSystemWindowInsetLeft(),
                            insets.getSystemWindowInsetTop(),
                            insets.getSystemWindowInsetRight(),
                            insets.getSystemWindowInsetBottom());
                    return WindowInsets.CONSUMED;
                });

        HorizontalScrollView scroller = new HorizontalScrollView(this);
        scroller.setHorizontalScrollBarEnabled(false);
        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        for (String tab : TABS) {
            Button b = new Button(this);
            b.setText(label(tab));
            b.setAllCaps(false);
            b.setOnClickListener(v -> select(tab, -1));
            bar.addView(b);
        }
        scroller.addView(bar);
        root.addView(scroller, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        content = new FrameLayout(this);
        root.addView(content, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        setContentView(root);
        notifyFrom(getIntent());
        select(tabFrom(getIntent()), rowFrom(getIntent()));
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        notifyFrom(intent);
        select(tabFrom(intent), rowFrom(intent));
    }

    private void notifyFrom(Intent intent) {
        String text = intent == null ? null : intent.getStringExtra("notify");
        if (text == null) {
            return;
        }
        NotificationManager nm = getSystemService(NotificationManager.class);
        nm.createNotificationChannel(
                new NotificationChannel("testbed", "Testbed", NotificationManager.IMPORTANCE_DEFAULT));
        nm.notify(1, new Notification.Builder(this, "testbed")
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentTitle("phone-use")
                .setContentText(text)
                .setStyle(new Notification.BigTextStyle().bigText(text))
                .build());
    }

    private static String tabFrom(Intent intent) {
        if (rowFrom(intent) >= 0) {
            return "list";
        }
        String t = intent == null ? null : intent.getStringExtra("tab");
        return t == null ? "taps" : t;
    }

    private static int rowFrom(Intent intent) {
        Uri u = intent == null ? null : intent.getData();
        if (u == null || !"list".equals(u.getHost()) || u.getPathSegments().isEmpty()) {
            return -1;
        }
        try {
            return Integer.parseInt(u.getPathSegments().get(0));
        } catch (NumberFormatException e) {
            return -1;
        }
    }

    static String label(String tab) {
        return Character.toUpperCase(tab.charAt(0)) + tab.substring(1);
    }

    private void select(String tab, int row) {
        if (tab.equals(current) && row < 0) {
            return;
        }
        current = tab;
        if (panel != null) {
            if (panel.isAttachedToWindow()) {
                getWindowManager().removeView(panel);
            }
            panel = null;
        }
        getWindow().clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
        content.removeAllViews();
        View screen;
        switch (tab) {
            case "text": screen = Screens.text(this); break;
            case "list": screen = Screens.list(this, row); break;
            case "gestures": screen = Screens.gestures(this); break;
            case "busy": screen = Screens.busy(this); break;
            case "secure":
                getWindow().setFlags(
                        WindowManager.LayoutParams.FLAG_SECURE,
                        WindowManager.LayoutParams.FLAG_SECURE);
                screen = Screens.secure(this);
                break;
            case "dialogs": screen = Screens.dialogs(this); break;
            case "web": screen = Screens.web(this); break;
            case "webopaque": screen = Screens.web(this, false); break;
            case "settle": screen = Screens.settle(this); break;
            case "wait": screen = Screens.waiting(this); break;
            case "checkout": screen = Screens.checkout(this); break;
            case "signup": screen = Buggy.signup(this); break;
            case "shop": screen = Buggy.shop(this); break;
            case "relay": screen = Buggy.relay(this, true); break;
            case "relaylog": screen = Buggy.relay(this, false); break;
            case "drain": screen = Buggy.drain(this); break;
            case "compose": screen = ComposeScreen.create(this); break;
            case "latefocus": screen = Screens.lateFocus(this); break;
            case "hollow":
                screen = new View(this);
                panel = Screens.hollow(this, content);
                break;
            default: screen = Screens.taps(this); break;
        }
        content.addView(screen, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT,
                Gravity.TOP));
        Events.emit(Events.put(Events.event("tab"), "name", tab));
    }
}
