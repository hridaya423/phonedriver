package dev.phoneuse.spikeapp;

import android.app.Activity;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.widget.ArrayAdapter;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ListView;
import android.widget.ProgressBar;
import android.widget.Switch;
import android.widget.TextView;

public class Main extends Activity {
    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        String mode = getIntent().getStringExtra("mode");
        if (mode == null) mode = "list";
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(24, 120, 24, 24);
        switch (mode) {
            case "list": {
                ListView lv = new ListView(this);
                String[] rows = new String[1000];
                for (int i = 0; i < rows.length; i++) rows[i] = "Row " + i;
                lv.setAdapter(new ArrayAdapter<String>(this, android.R.layout.simple_list_item_1, rows));
                root.addView(lv, new LinearLayout.LayoutParams(-1, -1));
                break;
            }
            case "secure": {
                getWindow().setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE);
                TextView t = new TextView(this);
                t.setText("SECRET code 123456");
                t.setTextSize(32);
                root.addView(t);
                break;
            }
            case "edit": {
                TextView t = new TextView(this);
                t.setText("Edit field below");
                root.addView(t);
                EditText e = new EditText(this);
                e.setHint("type here");
                e.setContentDescription("spike_edit");
                root.addView(e, new LinearLayout.LayoutParams(-1, -2));
                e.requestFocus();
                break;
            }
            case "switch": {
                Switch s = new Switch(this);
                s.setText("Toggle me");
                s.setTextSize(28);
                root.addView(s, new LinearLayout.LayoutParams(-1, 300));
                break;
            }
            case "touch": {
                final TextView t = new TextView(this);
                t.setTextSize(20);
                t.setText("touch log");
                t.setContentDescription("touchlog");
                root.addView(t, new LinearLayout.LayoutParams(-1, -2));
                final int[] c = new int[6];
                final float[] scale = {1f};
                final android.view.ScaleGestureDetector sg = new android.view.ScaleGestureDetector(this, new android.view.ScaleGestureDetector.SimpleOnScaleGestureListener() {
                    @Override
                    public boolean onScale(android.view.ScaleGestureDetector d) {
                        scale[0] *= d.getScaleFactor();
                        return true;
                    }
                });
                View v = new View(this);
                v.setBackgroundColor(0xFFCCDDFF);
                v.setOnTouchListener(new View.OnTouchListener() {
                    @Override
                    public boolean onTouch(View view, android.view.MotionEvent e) {
                        sg.onTouchEvent(e);
                        switch (e.getActionMasked()) {
                            case android.view.MotionEvent.ACTION_DOWN: c[0]++; break;
                            case android.view.MotionEvent.ACTION_UP: c[1]++; break;
                            case android.view.MotionEvent.ACTION_MOVE: c[2]++; break;
                            case android.view.MotionEvent.ACTION_POINTER_DOWN: c[3]++; break;
                            case android.view.MotionEvent.ACTION_POINTER_UP: c[4]++; break;
                        }
                        c[5] = Math.max(c[5], e.getPointerCount());
                        t.setText("down=" + c[0] + " up=" + c[1] + " move=" + c[2] + " pdown=" + c[3] + " pup=" + c[4] + " maxptr=" + c[5] + " scale=" + String.format("%.2f", scale[0]) + " last=" + (int) e.getX() + "," + (int) e.getY());
                        return true;
                    }
                });
                root.addView(v, new LinearLayout.LayoutParams(-1, -1));
                break;
            }
            case "dialog": {
                TextView t = new TextView(this);
                t.setText("Background page");
                root.addView(t);
                new android.app.AlertDialog.Builder(this).setTitle("Delete conversation?").setMessage("This removes 42 messages from this device.")
                        .setPositiveButton("Delete", null).setNegativeButton("Cancel", null).show();
                break;
            }
            case "spinner": {
                ProgressBar p = new ProgressBar(this);
                p.setIndeterminate(true);
                root.addView(p, new LinearLayout.LayoutParams(126, 126));
                break;
            }
        }
        setContentView(root);
    }
}
