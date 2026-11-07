package dev.phonedriver.agent;

import android.content.Context;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.net.Uri;
import com.genymobile.scrcpy.FakeContext;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

final class Apps {
    private static PackageManager packages() {
        return FakeContext.get().getPackageManager();
    }

    private static JSONObject app(String pkg, CharSequence label, boolean system) {
        JSONObject o = new JSONObject();
        Json.put(o, "package", pkg);
        Json.put(o, "label", label.toString());
        Json.put(o, "system", system);
        return o;
    }

    static JSONArray list(boolean all) {
        PackageManager pm = packages();
        JSONArray out = new JSONArray();
        if (all) {
            for (ApplicationInfo ai : pm.getInstalledApplications(0)) {
                out.put(app(ai.packageName, ai.loadLabel(pm), (ai.flags & ApplicationInfo.FLAG_SYSTEM) != 0));
            }
            return out;
        }
        Intent launcher = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER);
        for (ResolveInfo ri : pm.queryIntentActivities(launcher, 0)) {
            ApplicationInfo ai = ri.activityInfo.applicationInfo;
            out.put(app(ai.packageName, ri.loadLabel(pm), (ai.flags & ApplicationInfo.FLAG_SYSTEM) != 0));
        }
        return out;
    }

    private static void start(Intent intent) {
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        int result = ServiceManager.getActivityManager().startActivity(intent);
        if (result < 0) {
            throw new IllegalStateException("startActivity failed with " + result);
        }
    }

    static void launch(String pkg) {
        Intent intent = packages().getLaunchIntentForPackage(pkg);
        if (intent == null) {
            throw new IllegalArgumentException("no launcher activity for " + pkg);
        }
        start(intent);
    }

    static String openUrl(String url, String pkg) {
        Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
        if (pkg != null && !pkg.isEmpty()) {
            intent.setPackage(pkg);
        }
        ResolveInfo ri = packages().resolveActivity(intent, 0);
        if (ri == null) {
            throw new IllegalArgumentException("no app handles " + url);
        }
        start(intent);
        return ri.activityInfo.packageName;
    }

    private Apps() {}
}
