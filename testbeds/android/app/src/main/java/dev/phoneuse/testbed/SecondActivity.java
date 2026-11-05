package dev.phoneuse.testbed;

import android.app.Activity;
import android.os.Bundle;
import android.util.TypedValue;
import android.view.Gravity;
import android.widget.TextView;

public final class SecondActivity extends Activity {
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        TextView t = new TextView(this);
        t.setText("Second screen");
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, 24);
        t.setGravity(Gravity.CENTER);
        if (android.os.Build.VERSION.SDK_INT >= 28) {
            t.setAccessibilityHeading(true);
        }
        setContentView(t);
        Events.emit(Events.put(Events.event("second"), "state", "created"));
    }

    @Override
    protected void onDestroy() {
        Events.emit(Events.put(Events.event("second"), "state", "destroyed"));
        super.onDestroy();
    }
}
