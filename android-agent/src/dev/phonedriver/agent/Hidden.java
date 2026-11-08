package dev.phonedriver.agent;

import android.hardware.display.VirtualDisplay;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.os.Build;
import android.view.Surface;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import com.genymobile.scrcpy.wrappers.WindowManager;

final class Hidden {
    static final int EXIT_UNAVAILABLE = 5;
    private static final int MIN_API = 33;
    private static final int PUBLIC = 1;
    private static final int PRESENTATION = 1 << 1;
    private static final int OWN_CONTENT_ONLY = 1 << 3;
    private static final int SUPPORTS_TOUCH = 1 << 6;
    private static final int ROTATES_WITH_CONTENT = 1 << 7;
    private static final int DESTROY_CONTENT_ON_REMOVAL = 1 << 8;
    private static final int SHOULD_SHOW_SYSTEM_DECORATIONS = 1 << 9;
    private static final int TRUSTED = 1 << 10;
    private static final int OWN_DISPLAY_GROUP = 1 << 11;
    private static final int ALWAYS_UNLOCKED = 1 << 12;
    private static final int TOUCH_FEEDBACK_DISABLED = 1 << 13;
    private static final int OWN_FOCUS = 1 << 14;
    private static final int DEVICE_DISPLAY_GROUP = 1 << 15;
    private static final int[] SIZE_FALLBACK = {0, 2560, 1920, 1600, 1280, 1024, 800};

    private static VirtualDisplay display;
    private static MediaCodec codec;
    private static int id;
    private static int width;
    private static int height;

    static boolean active() {
        return display != null;
    }

    static int displayId() {
        return display == null ? 0 : id;
    }

    static MediaCodec codec() {
        return codec;
    }

    static int width() {
        return width;
    }

    static int height() {
        return height;
    }

    private static void unavailable(String why) {
        System.err.println("hidden display: " + why);
        System.exit(EXIT_UNAVAILABLE);
    }

    private static int flags() {
        // DESTROY_CONTENT_ON_REMOVAL: without it the apps on the display move to the phone's own screen when it goes.
        int flags = PUBLIC | PRESENTATION | OWN_CONTENT_ONLY | SUPPORTS_TOUCH | ROTATES_WITH_CONTENT
                | DESTROY_CONTENT_ON_REMOVAL | SHOULD_SHOW_SYSTEM_DECORATIONS
                | TRUSTED | OWN_DISPLAY_GROUP | ALWAYS_UNLOCKED | TOUCH_FEEDBACK_DISABLED;
        if (Build.VERSION.SDK_INT >= 34) {
            flags |= OWN_FOCUS | DEVICE_DISPLAY_GROUP;
        }
        return flags;
    }

    static void open() throws Exception {
        if (Build.VERSION.SDK_INT < MIN_API) {
            unavailable("needs Android 13 (API 33) or newer, this is API " + Build.VERSION.SDK_INT);
        }
        DisplayInfo main = ServiceManager.getDisplayManager().getDisplayInfo(0);
        int mainW = main.getSize().getWidth();
        int mainH = main.getSize().getHeight();
        MediaCodec encoder = MediaCodec.createEncoderByType("video/avc");
        MediaCodecInfo.VideoCapabilities caps =
                encoder.getCodecInfo().getCapabilitiesForType("video/avc").getVideoCapabilities();
        int align = Math.max(caps.getWidthAlignment(), caps.getHeightAlignment());
        Surface surface = null;
        int[] size = null;
        Exception last = null;
        for (int maxSize : SIZE_FALLBACK) {
            size = Video.fit(mainW, mainH, maxSize, align);
            try {
                encoder.configure(Video.format(size[0], size[1]), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
                surface = encoder.createInputSurface();
                break;
            } catch (Exception e) {
                last = e;
                System.err.println("encoder rejected " + size[0] + "x" + size[1] + ": " + e);
                encoder.reset();
            }
        }
        if (surface == null) {
            unavailable("no encoder size works: " + last);
        }
        int dpi = Math.round(main.getDpi() * (float) size[0] / mainW);
        try {
            display = ServiceManager.getDisplayManager()
                    .createNewVirtualDisplay("phonedriver", size[0], size[1], dpi, surface, flags());
        } catch (Exception e) {
            Throwable cause = e instanceof java.lang.reflect.InvocationTargetException && e.getCause() != null ? e.getCause() : e;
            unavailable("the system refused a virtual display: " + cause);
        }
        id = display.getDisplay().getDisplayId();
        // A display that shows system decorations keeps its own keyboard; otherwise the keyboard opens on the phone's screen.
        ServiceManager.getWindowManager().setDisplayImePolicy(id, WindowManager.DISPLAY_IME_POLICY_LOCAL);
        encoder.start();
        codec = encoder;
        width = size[0];
        height = size[1];
        System.err.println("hidden display " + id + ": " + width + "x" + height + " at " + dpi + " dpi");
    }

    private Hidden() {}
}
