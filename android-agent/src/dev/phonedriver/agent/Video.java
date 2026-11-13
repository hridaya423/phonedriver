package dev.phonedriver.agent;

import android.graphics.Rect;
import android.hardware.display.VirtualDisplay;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaFormat;
import android.os.Bundle;
import android.os.IBinder;
import android.view.Surface;
import com.genymobile.scrcpy.display.DisplayInfo;
import com.genymobile.scrcpy.wrappers.ServiceManager;
import com.genymobile.scrcpy.wrappers.SurfaceControl;
import java.io.DataOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.util.concurrent.ConcurrentLinkedQueue;

final class Video {
    private static final int[] MAX_SIZE_FALLBACK = {2560, 1920, 1600, 1280, 1024, 800};
    private static final int FOURCC_H264 = 0x68323634;
    private static final long DISPLAY_POLL_NS = 100_000_000L;
    private static final int FLAG_BARRIER = 4;
    private static final int OUTPUT_WAIT_US = 10_000;
    private static volatile boolean syncRequested;
    private static final ConcurrentLinkedQueue<Long> barriers = new ConcurrentLinkedQueue<>();
    private static final int BIT_RATE = 8_000_000;
    private static final int FPS = 30;
    private static volatile int bitRate = BIT_RATE;
    private static volatile int fps = FPS;
    private static volatile boolean constantRate;

    private final DataOutputStream out;
    private int maxSize;

    Video(DataOutputStream out, int maxSize, int bitRate, int fps) {
        this.out = out;
        this.maxSize = maxSize;
        Video.bitRate = bitRate > 0 ? bitRate : BIT_RATE;
        Video.fps = fps > 0 ? fps : FPS;
    }

    static void requestSyncFrame() {
        syncRequested = true;
    }

    static void requestBarrier(long id) {
        barriers.add(id);
    }

    static int[] fit(int w, int h, int maxSize, int align) {
        if (maxSize > 0 && Math.max(w, h) > maxSize) {
            double s = (double) maxSize / Math.max(w, h);
            w = (int) Math.round(w * s);
            h = (int) Math.round(h * s);
        }
        return new int[] {w & ~(align - 1), h & ~(align - 1)};
    }

    static MediaFormat format(int w, int h) {
        MediaFormat f = new MediaFormat();
        f.setString(MediaFormat.KEY_MIME, "video/avc");
        f.setInteger(MediaFormat.KEY_BIT_RATE, bitRate);
        if (bitRate != BIT_RATE && constantRate) {
            f.setInteger(MediaFormat.KEY_BITRATE_MODE, MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR);
        }
        f.setInteger(MediaFormat.KEY_FRAME_RATE, 60);
        f.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
        f.setInteger(MediaFormat.KEY_COLOR_RANGE, MediaFormat.COLOR_RANGE_LIMITED);
        f.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 10);
        f.setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, 100_000);
        f.setInteger(MediaFormat.KEY_PRIORITY, 0);
        f.setInteger(MediaFormat.KEY_LATENCY, 1);
        f.setFloat("max-fps-to-encoder", (float) fps);
        f.setInteger(MediaFormat.KEY_WIDTH, w);
        f.setInteger(MediaFormat.KEY_HEIGHT, h);
        return f;
    }

    private void runHidden() throws Exception {
        out.writeInt(FOURCC_H264);
        out.writeInt(Hidden.width());
        out.writeInt(Hidden.height());
        out.flush();
        pump(Hidden.codec(), -1, 0, 0, 0);
    }

    void run() throws Exception {
        if (Hidden.active()) {
            runHidden();
            return;
        }
        MediaCodec codec = MediaCodec.createEncoderByType("video/avc");
        MediaCodecInfo.VideoCapabilities caps =
                codec.getCodecInfo().getCapabilitiesForType("video/avc").getVideoCapabilities();
        int align = Math.max(caps.getWidthAlignment(), caps.getHeightAlignment());
        // Asked for 400 kbit/s without a rate mode, the HONOR X8's encoder sent 853 kbit/s of its lock screen.
        constantRate = codec.getCodecInfo().getCapabilitiesForType("video/avc").getEncoderCapabilities()
                .isBitrateModeSupported(MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR);
        boolean first = true;
        int fallback = -1;
        while (true) {
            DisplayInfo di = ServiceManager.getDisplayManager().getDisplayInfo(0);
            int dw = di.getSize().getWidth();
            int dh = di.getSize().getHeight();
            int rotation = di.getRotation();
            int[] size = fit(dw, dh, maxSize, align);
            Surface surface = null;
            VirtualDisplay display = null;
            IBinder legacyDisplay = null;
            boolean started = false;
            try {
                codec.configure(format(size[0], size[1]), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
                surface = codec.createInputSurface();
                try {
                    display = ServiceManager.getDisplayManager().createVirtualDisplay("phonedriver", size[0], size[1], 0, surface);
                } catch (Exception e) {
                    legacyDisplay = SurfaceControl.createDisplay("phonedriver", false);
                    SurfaceControl.openTransaction();
                    try {
                        SurfaceControl.setDisplaySurface(legacyDisplay, surface);
                        SurfaceControl.setDisplayProjection(legacyDisplay, 0, new Rect(0, 0, dw, dh), new Rect(0, 0, size[0], size[1]));
                        SurfaceControl.setDisplayLayerStack(legacyDisplay, di.getLayerStack());
                    } finally {
                        SurfaceControl.closeTransaction();
                    }
                }
                codec.start();
                started = true;
                if (!first) {
                    out.writeLong(-1L);
                    out.writeByte(0);
                    out.writeInt(0);
                }
                first = false;
                out.writeInt(FOURCC_H264);
                out.writeInt(size[0]);
                out.writeInt(size[1]);
                out.flush();
                pump(codec, 0, dw, dh, rotation);
            } catch (IOException e) {
                throw e;
            } catch (Exception e) {
                if (started || fallback + 1 >= MAX_SIZE_FALLBACK.length) {
                    throw e;
                }
                fallback++;
                maxSize = MAX_SIZE_FALLBACK[fallback];
                System.err.println("encoder rejected " + size[0] + "x" + size[1] + ": " + e + "; retrying with max size " + maxSize);
            } finally {
                try {
                    if (started) {
                        codec.stop();
                    }
                } catch (Exception ignored) {
                }
                codec.reset();
                if (display != null) {
                    display.release();
                }
                if (legacyDisplay != null) {
                    SurfaceControl.destroyDisplay(legacyDisplay);
                }
                if (surface != null) {
                    surface.release();
                }
            }
        }
    }

    private void pump(MediaCodec codec, int displayId, int dw, int dh, int rotation) throws IOException {
        MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
        long lastCheck = 0;
        while (true) {
            if (syncRequested) {
                syncRequested = false;
                Bundle b = new Bundle();
                b.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0);
                codec.setParameters(b);
            }
            if (!barriers.isEmpty()) {
                int ready;
                while ((ready = codec.dequeueOutputBuffer(info, 0)) >= 0) {
                    writeOutput(codec, ready, info);
                }
                Long barrier;
                while ((barrier = barriers.poll()) != null) {
                    out.writeLong(0);
                    out.writeByte(FLAG_BARRIER);
                    out.writeInt(8);
                    out.writeLong(barrier);
                    out.flush();
                }
            }
            int id = codec.dequeueOutputBuffer(info, OUTPUT_WAIT_US);
            long now = System.nanoTime();
            if (displayId >= 0 && now - lastCheck > DISPLAY_POLL_NS) {
                lastCheck = now;
                DisplayInfo d = ServiceManager.getDisplayManager().getDisplayInfo(displayId);
                if (d.getSize().getWidth() != dw || d.getSize().getHeight() != dh || d.getRotation() != rotation) {
                    if (id >= 0) {
                        codec.releaseOutputBuffer(id, false);
                    }
                    return;
                }
            }
            if (id < 0) {
                continue;
            }
            writeOutput(codec, id, info);
        }
    }

    private void writeOutput(MediaCodec codec, int id, MediaCodec.BufferInfo info) throws IOException {
        try {
            if (info.size > 0) {
                ByteBuffer buf = codec.getOutputBuffer(id);
                byte[] data = new byte[info.size];
                buf.position(info.offset);
                buf.get(data, 0, info.size);
                int flags = 0;
                if ((info.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) != 0) {
                    flags |= 1;
                }
                if ((info.flags & MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0) {
                    flags |= 2;
                }
                out.writeLong(info.presentationTimeUs);
                out.writeByte(flags);
                out.writeInt(data.length);
                out.write(data);
                out.flush();
            }
        } finally {
            codec.releaseOutputBuffer(id, false);
        }
    }
}
