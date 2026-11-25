package dev.phonedriver.testbed;

import android.os.Bundle;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

import androidx.fragment.app.FragmentActivity;

import com.google.android.material.datepicker.MaterialDatePicker;
import com.google.android.material.timepicker.MaterialTimePicker;
import com.google.android.material.timepicker.TimeFormat;

import java.time.Instant;
import java.time.ZoneOffset;
import java.util.Locale;

/** The Material Components (View) date and time pickers, each opened on 2026-01-15 or 09:05. */
public final class MaterialPickers extends FragmentActivity {
    private TextView shown;

    private void picked(String field, String value) {
        shown.setText(field + " = " + value);
        Pickers.picked(field, value);
    }

    private Button button(String text, Runnable open) {
        Button b = new Button(this);
        b.setText(text);
        b.setAllCaps(false);
        b.setOnClickListener(v -> open.run());
        return b;
    }

    private void date(String field, int mode) {
        MaterialDatePicker<Long> p =
                MaterialDatePicker.Builder.datePicker()
                        .setSelection(1768435200000L)
                        .setInputMode(mode)
                        .build();
        p.addOnPositiveButtonClickListener(
                at -> picked(field, Instant.ofEpochMilli(at).atZone(ZoneOffset.UTC).toLocalDate().toString()));
        p.show(getSupportFragmentManager(), field);
    }

    private void time(String field, int format, int mode) {
        MaterialTimePicker p =
                new MaterialTimePicker.Builder()
                        .setTimeFormat(format)
                        .setHour(9)
                        .setMinute(5)
                        .setInputMode(mode)
                        .build();
        p.addOnPositiveButtonClickListener(
                v -> picked(field, String.format(Locale.ROOT, "%02d:%02d", p.getHour(), p.getMinute())));
        p.show(getSupportFragmentManager(), field);
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setFitsSystemWindows(true);
        shown = new TextView(this);
        shown.setText("nothing picked");
        root.addView(shown);
        root.addView(button("MDC date", () -> date("mdc_date", MaterialDatePicker.INPUT_MODE_CALENDAR)));
        root.addView(button("MDC date typed", () -> date("mdc_date_typed", MaterialDatePicker.INPUT_MODE_TEXT)));
        root.addView(
                button("MDC time", () -> time("mdc_time", TimeFormat.CLOCK_12H, MaterialTimePicker.INPUT_MODE_CLOCK)));
        root.addView(
                button(
                        "MDC time 24h",
                        () -> time("mdc_time_24h", TimeFormat.CLOCK_24H, MaterialTimePicker.INPUT_MODE_CLOCK)));
        root.addView(
                button(
                        "MDC time typed",
                        () -> time("mdc_time_typed", TimeFormat.CLOCK_12H, MaterialTimePicker.INPUT_MODE_KEYBOARD)));
        setContentView(root);
    }
}
