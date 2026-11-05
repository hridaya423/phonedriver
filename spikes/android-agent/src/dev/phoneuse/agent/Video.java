package dev.phoneuse.agent;

import android.graphics.Rect;
import android.hardware.display.VirtualDisplay;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaFormat;
import android.os.Build;
import android.os.Bundle;
import android.os.IBinder;
import android.os.Looper;
import android.view.Surface;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import com.genymobile.scrcpy.wrappers.SurfaceControl;
import java.io.DataOutputStream;
import java.nio.ByteBuffer;

final class Video implements Runnable {
    static final int[] MAX_SIZE_FALLBACK = {2560, 1920, 1600, 1280, 1024, 800};
    final DataOutputStream out;
    static volatile boolean syncRequested;
    static volatile String profileOverride = System.getenv("PROFILE");
    int maxSize = 0;

    Video(DataOutputStream out) {
        this.out = out;
    }

    static int[] fit(int w, int h, int maxSize, int align) {
        if (maxSize > 0 && Math.max(w, h) > maxSize) {
            double s = (double) maxSize / Math.max(w, h);
            w = (int) Math.round(w * s);
            h = (int) Math.round(h * s);
        }
        return new int[] {w & ~(align - 1), h & ~(align - 1)};
    }

    @Override
    public void run() {
        if (Looper.myLooper() == null) Looper.prepare();
        try {
            loop();
        } catch (Throwable t) {
            System.err.println("video ended: " + t);
        }
    }

    MediaFormat format(int w, int h) {
        MediaFormat f = new MediaFormat();
        f.setString(MediaFormat.KEY_MIME, "video/avc");
        f.setInteger(MediaFormat.KEY_BIT_RATE, 8_000_000);
        f.setInteger(MediaFormat.KEY_FRAME_RATE, 60);
        f.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
        f.setInteger(MediaFormat.KEY_COLOR_RANGE, MediaFormat.COLOR_RANGE_LIMITED);
        f.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 10);
        f.setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, 100_000);
        f.setInteger(MediaFormat.KEY_PRIORITY, 0);
        f.setInteger(MediaFormat.KEY_LATENCY, 1);
        f.setFloat("max-fps-to-encoder", 30f);
        f.setInteger(MediaFormat.KEY_WIDTH, w);
        f.setInteger(MediaFormat.KEY_HEIGHT, h);
        if (profileOverride != null && profileOverride.equals("baseline")) {
            f.setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileConstrainedBaseline);
        }
        return f;
    }

    void loop() throws Exception {
        MediaCodec codec = MediaCodec.createEncoderByType("video/avc");
        System.err.println("encoder: " + codec.getName());
        MediaCodecInfo.VideoCapabilities caps = codec.getCodecInfo().getCapabilitiesForType("video/avc").getVideoCapabilities();
        int align = Math.max(caps.getWidthAlignment(), caps.getHeightAlignment());
        System.err.println("alignment " + align);
        boolean first = true;
        int fallback = -1;
        while (true) {
            DisplayInfo di = ServiceManager.getDisplayManager().getDisplayInfo(0);
            int dw = di.getSize().getWidth(), dh = di.getSize().getHeight(), rot = di.getRotation();
            int[] sz = fit(dw, dh, maxSize, align);
            Surface surface = null;
            VirtualDisplay vd = null;
            IBinder disp = null;
            boolean started = false;
            try {
                codec.configure(format(sz[0], sz[1]), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
                surface = codec.createInputSurface();
                try {
                    vd = ServiceManager.getDisplayManager().createVirtualDisplay("phoneuse", sz[0], sz[1], 0, surface);
                    System.err.println("display via DisplayManager");
                } catch (Exception e) {
                    disp = SurfaceControl.createDisplay("phoneuse", false);
                    SurfaceControl.openTransaction();
                    try {
                        SurfaceControl.setDisplaySurface(disp, surface);
                        SurfaceControl.setDisplayProjection(disp, 0, new Rect(0, 0, dw, dh), new Rect(0, 0, sz[0], sz[1]));
                        SurfaceControl.setDisplayLayerStack(disp, di.getLayerStack());
                    } finally {
                        SurfaceControl.closeTransaction();
                    }
                    System.err.println("display via SurfaceControl (" + e + ")");
                }
                codec.start();
                started = true;
                if (first) {
                    out.writeInt(0x68323634);
                    first = false;
                } else {
                    out.writeLong(-1L);
                    out.writeByte(0);
                    out.writeInt(0);
                    out.writeInt(0x68323634);
                }
                out.writeInt(sz[0]);
                out.writeInt(sz[1]);
                out.flush();
                System.err.println("streaming " + sz[0] + "x" + sz[1] + " display " + dw + "x" + dh + " rot " + rot);
                MediaCodec.BufferInfo bi = new MediaCodec.BufferInfo();
                long lastCheck = 0;
                while (true) {
                    if (syncRequested) {
                        syncRequested = false;
                        Bundle b = new Bundle();
                        b.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0);
                        codec.setParameters(b);
                    }
                    int id = codec.dequeueOutputBuffer(bi, 50_000);
                    long now = System.nanoTime();
                    if (now - lastCheck > 100_000_000L) {
                        lastCheck = now;
                        DisplayInfo d2 = ServiceManager.getDisplayManager().getDisplayInfo(0);
                        if (d2.getSize().getWidth() != dw || d2.getSize().getHeight() != dh || d2.getRotation() != rot) {
                            if (id >= 0) codec.releaseOutputBuffer(id, false);
                            System.err.println("display changed, restarting");
                            break;
                        }
                    }
                    if (id < 0) continue;
                    try {
                        if (bi.size > 0) {
                            ByteBuffer buf = codec.getOutputBuffer(id);
                            byte[] data = new byte[bi.size];
                            buf.position(bi.offset);
                            buf.get(data, 0, bi.size);
                            int flags = 0;
                            if ((bi.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) != 0) flags |= 1;
                            if ((bi.flags & MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0) flags |= 2;
                            out.writeLong(bi.presentationTimeUs);
                            out.writeByte(flags);
                            out.writeInt(data.length);
                            out.write(data);
                            out.flush();
                        }
                    } finally {
                        codec.releaseOutputBuffer(id, false);
                    }
                }
            } catch (java.io.IOException e) {
                throw e;
            } catch (Exception e) {
                System.err.println("encode error " + e);
                if (fallback + 1 < MAX_SIZE_FALLBACK.length && !started) {
                    fallback++;
                    maxSize = MAX_SIZE_FALLBACK[fallback];
                    System.err.println("retry with max size " + maxSize);
                } else throw e;
            } finally {
                try {
                    if (started) codec.stop();
                } catch (Exception ignored) {
                }
                codec.reset();
                if (vd != null) vd.release();
                if (disp != null) SurfaceControl.destroyDisplay(disp);
                if (surface != null) surface.release();
            }
        }
    }
}
