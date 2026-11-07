package dev.phoneuse.testbed;

import android.app.Activity;
import android.content.Context;
import android.text.InputType;
import android.util.TypedValue;
import android.view.View;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.util.Locale;
import java.util.Random;

final class Buggy {
    private static LinearLayout column(Context c) {
        LinearLayout l = new LinearLayout(c);
        l.setOrientation(LinearLayout.VERTICAL);
        l.setPadding(32, 32, 32, 32);
        return l;
    }

    private static TextView line(Context c, String text, int sp) {
        TextView t = new TextView(c);
        t.setText(text);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sp);
        return t;
    }

    private static Button button(Context c, String text, View.OnClickListener l) {
        Button b = new Button(c);
        b.setText(text);
        b.setAllCaps(false);
        b.setOnClickListener(l);
        return b;
    }

    private static EditText input(Context c, String hint, int type) {
        EditText e = new EditText(c);
        e.setHint(hint);
        e.setSingleLine(true);
        e.setInputType(InputType.TYPE_CLASS_TEXT | type);
        return e;
    }

    private static void emit(String ev, Object... kv) {
        org.json.JSONObject o = Events.event(ev);
        for (int i = 0; i < kv.length; i += 2) {
            Events.put(o, (String) kv[i], kv[i + 1]);
        }
        Events.emit(o);
    }

    static View signup(Activity a) {
        LinearLayout root = column(a);
        root.addView(line(a, "Create your account", 22));
        EditText email = input(a, "Email", InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        EditText password = input(a, "Password", InputType.TYPE_TEXT_VARIATION_PASSWORD);
        EditText confirm = input(a, "Confirm password", InputType.TYPE_TEXT_VARIATION_PASSWORD);
        CheckBox terms = new CheckBox(a);
        terms.setText("I accept the terms");
        TextView status = line(a, "", 18);
        int[] taps = {0};
        boolean[] rejected = {false};
        Button create = button(a, "Create account", v -> {
            taps[0]++;
            String err = null;
            if (!email.getText().toString().contains("@")) {
                err = "Enter a valid email";
            } else if (password.getText().length() < 8) {
                err = "Password must be at least 8 characters";
            } else if (!password.getText().toString().equals(confirm.getText().toString())) {
                err = "Passwords do not match";
            } else if (!terms.isChecked()) {
                err = "Please accept the terms";
            }
            if (rejected[0] && err == null) {
                emit("signup_tap", "n", taps[0], "accepted", false, "ignored", true);
                rejected[0] = false;
                return;
            }
            if (err != null) {
                rejected[0] = true;
                status.setText(err);
                emit("signup_tap", "n", taps[0], "accepted", false, "error", err);
                return;
            }
            String shown = email.getText().toString();
            shown = shown.substring(0, shown.length() - 1);
            status.setText("Welcome, " + shown + "! Your account is ready.");
            emit("signup_tap", "n", taps[0], "accepted", true, "email", email.getText().toString());
        });
        root.addView(email);
        root.addView(password);
        root.addView(confirm);
        root.addView(terms);
        root.addView(create);
        root.addView(status);
        return root;
    }

    static View shop(Activity a) {
        LinearLayout root = column(a);
        root.addView(line(a, "Pad thai  $12.00", 22));
        TextView qtyView = line(a, "Quantity: 1", 20);
        TextView total = line(a, "Total: $12.00", 20);
        TextView promoView = line(a, "", 18);
        int[] qty = {1};
        double[] discount = {0};
        Button minus = button(a, "Decrease quantity", v -> {
            if (qty[0] > 1) {
                qty[0]--;
            }
            qtyView.setText("Quantity: " + qty[0]);
            emit("shop_qty", "qty", qty[0], "total_shown", total.getText().toString());
        });
        Button plus = button(a, "Increase quantity", v -> {
            qty[0]++;
            qtyView.setText("Quantity: " + qty[0]);
            total.setText(String.format(Locale.US, "Total: $%.2f", qty[0] * 12.0 - discount[0]));
            emit("shop_qty", "qty", qty[0], "total_shown", total.getText().toString());
        });
        EditText code = input(a, "Promo code", 0);
        Button apply = button(a, "Apply code", v -> {
            String c = code.getText().toString().trim();
            if (c.equalsIgnoreCase("SAVE10")) {
                discount[0] = 10.0;
                promoView.setText("SAVE10 applied: 10% off");
                total.setText(String.format(Locale.US, "Total: $%.2f", qty[0] * 12.0 - discount[0]));
            } else {
                promoView.setText("Invalid code");
            }
            emit("shop_promo", "code", c, "total_shown", total.getText().toString());
        });
        Button order = button(a, "Place order", v ->
                emit("shop_order", "qty", qty[0], "total_shown", total.getText().toString()));
        root.addView(qtyView);
        root.addView(plus);
        root.addView(minus);
        root.addView(code);
        root.addView(apply);
        root.addView(promoView);
        root.addView(total);
        root.addView(order);
        return root;
    }

    static View relay(Activity a, boolean onScreen) {
        File f = new File(a.getFilesDir(), "relay_code");
        String code;
        try {
            if (f.exists()) {
                try (java.util.Scanner s = new java.util.Scanner(f)) {
                    code = s.nextLine();
                }
            } else {
                code = String.valueOf(1000000 + new Random().nextInt(9000000));
                try (FileWriter w = new FileWriter(f)) {
                    w.write(code + "\n");
                }
            }
        } catch (IOException e) {
            throw new IllegalStateException(e);
        }
        LinearLayout root = column(a);
        root.addView(line(a, "Locker pickup", 22));
        if (onScreen) {
            root.addView(line(a, "Your pickup code: " + code, 28));
        } else {
            root.addView(line(a, "Your pickup code was sent to the device log.", 20));
            emit("pickup_code", "code", code);
        }
        return root;
    }

    private static android.os.PowerManager.WakeLock drainLock;

    static View drain(Activity a) {
        android.os.PowerManager pm = (android.os.PowerManager) a.getSystemService(Context.POWER_SERVICE);
        if (drainLock == null) {
            drainLock = pm.newWakeLock(android.os.PowerManager.PARTIAL_WAKE_LOCK, "jobs:drain");
            drainLock.acquire();
        }
        LinearLayout root = column(a);
        root.addView(line(a, "Background sync running", 22));
        return root;
    }

    private Buggy() {}
}
