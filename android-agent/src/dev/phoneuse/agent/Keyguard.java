package dev.phoneuse.agent;

import android.os.IBinder;
import android.os.IInterface;
import java.lang.reflect.Method;

final class Keyguard {
    private static final int USER_SYSTEM = 0;
    private static IInterface windowManager;

    // KeyguardManager cannot be created from app_process on API 37 ("ApplicationSharedMemory not initialized"),
    // so ask IWindowManager directly.
    private static synchronized IInterface windowManager() throws ReflectiveOperationException {
        if (windowManager == null) {
            IBinder binder = (IBinder) Class.forName("android.os.ServiceManager")
                    .getDeclaredMethod("getService", String.class).invoke(null, "window");
            windowManager = (IInterface) Class.forName("android.view.IWindowManager$Stub")
                    .getMethod("asInterface", IBinder.class).invoke(null, binder);
        }
        return windowManager;
    }

    private static boolean call(String name) throws ReflectiveOperationException {
        IInterface wm = windowManager();
        for (Method m : wm.getClass().getMethods()) {
            if (!m.getName().equals(name)) {
                continue;
            }
            if (m.getParameterTypes().length == 0) {
                return (Boolean) m.invoke(wm);
            }
            if (m.getParameterTypes().length == 1 && m.getParameterTypes()[0] == int.class) {
                return (Boolean) m.invoke(wm, USER_SYSTEM);
            }
        }
        throw new NoSuchMethodException(name);
    }

    static boolean locked() {
        try {
            return call("isKeyguardLocked");
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    static boolean secure() {
        try {
            return call("isKeyguardSecure");
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    private Keyguard() {}
}
