package dev.phonedriver.agent;

import android.os.IBinder;

import org.json.JSONArray;
import org.json.JSONObject;

import java.text.DecimalFormatSymbols;
import java.text.SimpleDateFormat;
import java.util.Calendar;
import java.util.GregorianCalendar;
import java.util.Locale;

/** The names of the months and of the halves of the day in the language an app is drawn in. */
final class Names {
    static JSONObject of(String app) throws Exception {
        Locale locale = locale(app);
        JSONObject r = new JSONObject();
        r.put("locale", locale.toLanguageTag());
        // English follows the language's own forms: Compose Material 3 writes `AM` and `PM` in Japanese, where
        // the device says 午前 and 午後 (API 37 emulator).
        Locale[] both = {locale, Locale.ENGLISH};
        JSONArray months = new JSONArray();
        for (int month = 0; month < 12; month++) {
            months.put(forms(both, new GregorianCalendar(2001, month, 15, 9, 0), "MMMM", "LLLL", "MMM", "LLL"));
        }
        r.put("months", months);
        r.put("am", forms(both, new GregorianCalendar(2001, 0, 15, 9, 0), "a"));
        r.put("pm", forms(both, new GregorianCalendar(2001, 0, 15, 21, 0), "a"));
        char zero = DecimalFormatSymbols.getInstance(locale).getZeroDigit();
        StringBuilder digits = new StringBuilder();
        for (int d = 0; zero != '0' && d < 10; d++) {
            digits.append((char) (zero + d));
        }
        r.put("digits", digits.toString());
        return r;
    }

    private static JSONArray forms(Locale[] locales, Calendar at, String... patterns) {
        JSONArray all = new JSONArray();
        for (Locale locale : locales) {
            for (String pattern : patterns) {
                all.put(new SimpleDateFormat(pattern, locale).format(at.getTime()));
            }
        }
        return all;
    }

    /**
     * The app's own language when the person set one for it (Android 13 and later), else the system's as it is
     * now: this process started with the language of that moment and is not told when it changes.
     */
    private static Locale locale(String app) {
        try {
            if (app != null) {
                IBinder binder =
                        (IBinder)
                                Class.forName("android.os.ServiceManager")
                                        .getMethod("getService", String.class)
                                        .invoke(null, "locale");
                Object manager =
                        Class.forName("android.app.ILocaleManager$Stub")
                                .getMethod("asInterface", IBinder.class)
                                .invoke(null, binder);
                int user = (Integer) Class.forName("android.app.ActivityManager").getMethod("getCurrentUser").invoke(null);
                Object list =
                        manager.getClass()
                                .getMethod("getApplicationLocales", String.class, int.class)
                                .invoke(manager, app, user);
                if (list != null && !((android.os.LocaleList) list).isEmpty()) {
                    return ((android.os.LocaleList) list).get(0);
                }
            }
        } catch (Throwable ignored) {
        }
        try {
            Object am = Class.forName("android.app.ActivityManager").getMethod("getService").invoke(null);
            android.content.res.Configuration c =
                    (android.content.res.Configuration) am.getClass().getMethod("getConfiguration").invoke(am);
            if (!c.getLocales().isEmpty()) {
                return c.getLocales().get(0);
            }
        } catch (Throwable ignored) {
        }
        return Locale.getDefault();
    }

    private Names() {}
}
