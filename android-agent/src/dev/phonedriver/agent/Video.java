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

final class Video {
    private static final int[] MAX_SIZE_FALLBACK = {2560, 1920, 1600, 1280, 1024, 800};
    private static final int FOURCC_H264 = 0x68323634;
    private static final long DISPLAY_POLL_NS = 100_000_000L;
    private static final int FLAG_BARRIER = 4;
    private static final int OUTPUT_WAIT_US = 10_000;

    private static final int BIT_RATE = 8_000_000;
    static final int FPS = 30;

    private static final long REPEAT_US = 100_000L;

    // A phone whose display HAL fails a virtual display that renders straight into the encoder gives no output at
    // all (Galaxy A53, One UI 8, #22). A software encoder is left out: on a loaded API 37 emulator its first frame
    // came up to 5 s late.
    private static final long NO_OUTPUT_NS = 1_500_000_000L;
    private static volatile boolean relayFallback;

    // A frame's presentation time is the System.nanoTime at which the screen showed it: on the API 37 emulator every
    // frame left the encoder between 30 ms and 5 s after its own. An encoder whose times are further off than this
    // is on another clock; its frames are not timed, and a barrier then passes once the output on hand is written.
    private static final long CLOCK_LIMIT_US = 60_000_000L;

    private final DataOutputStream out;
    private final Lane lane = Lane.here();
    private long wroteAtUs;
    private long wrotePtsUs;
    private boolean unsettled;
    private int maxSize;
    private final int bitRate;
    private final int fps;

    Video(DataOutputStream out, int maxSize, int bitRate, int fps) {
        this.out = out;
        this.maxSize = maxSize;
        this.bitRate = bitRate > 0 ? bitRate : BIT_RATE;
        this.fps = fps > 0 ? fps : FPS;
    }

    static void requestSyncFrame() {
        Lane.here().syncRequested = true;
    }

    /** Milliseconds the barrier may take longer than usual, because the encoder is that far behind the screen. */
    static long requestBarrier(long id) {
        Lane lane = Lane.here();
        lane.barriers.add(new long[] {id, System.nanoTime() / 1000});
        long lag = lane.videoLagUs;
        return lag > REPEAT_US ? (REPEAT_US + 2 * lag) / 1000 : 0;
    }

    // Two lanes on the API 37 emulator, both animating after a tap: the software encoder stood still for up to 611 ms
    // at a time, the frames queued, and each came out up to 1.45 s after the screen had shown it. A barrier written
    // then, after the output on hand, was followed by the frames of the tap, and four screenshots in five showed the
    // screen before it. An encoder that keeps up is not waited for. One that is behind is waited for until it gives
    // a frame shown after the barrier was asked for, which a screen that changed in the last second has, because
    // the last frame is repeated for about that long; or until it has given nothing for longer than it was behind.
    private boolean encodedUpTo(long askedUs) {
        long lag = lane.videoLagUs;
        return lag <= REPEAT_US
                || wrotePtsUs >= askedUs
                || System.nanoTime() / 1000 - wroteAtUs >= REPEAT_US + 2 * lag;
    }

    static int[] fit(int w, int h, int maxSize, int align) {
        if (maxSize > 0 && Math.max(w, h) > maxSize) {
            double s = (double) maxSize / Math.max(w, h);
            w = (int) Math.round(w * s);
            h = (int) Math.round(h * s);
        }
        return new int[] {w & ~(align - 1), h & ~(align - 1)};
    }

    /** The format of a hidden display's stream: its encoder starts before the video connection that could ask for less. */
    static MediaFormat format(int w, int h, int fps) {
        return format(w, h, BIT_RATE, fps, false);
    }

    private static MediaFormat format(int w, int h, int bitRate, int fps, boolean constantRate) {
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
        f.setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, REPEAT_US);
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
        pump(Hidden.codec(), -1, 0, 0, 0, false);
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
        boolean constantRate = codec.getCodecInfo().getCapabilitiesForType("video/avc").getEncoderCapabilities()
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
            GlRelay relay = null;
            VirtualDisplay display = null;
            IBinder legacyDisplay = null;
            boolean started = false;
            try {
                codec.configure(format(size[0], size[1], bitRate, fps, constantRate), null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
                surface = codec.createInputSurface();
                if (relayFallback || GlRelay.wanted()) {
                    try {
                        relay = GlRelay.start(size[0], size[1], surface);
                    } catch (Exception e) {
                        System.err.println("video: no OpenGL relay, rendering straight to the encoder: " + e);
                    }
                }
                Surface target = relay != null ? relay.input() : surface;
                try {
                    display = ServiceManager.getDisplayManager().createVirtualDisplay("phonedriver", size[0], size[1], 0, target);
                } catch (Exception e) {
                    legacyDisplay = SurfaceControl.createDisplay("phonedriver", false);
                    SurfaceControl.openTransaction();
                    try {
                        SurfaceControl.setDisplaySurface(legacyDisplay, target);
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
                pump(codec, 0, dw, dh, rotation, relay == null && !software(codec));
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
                if (relay != null) {
                    relay.release();
                }
                if (surface != null) {
                    surface.release();
                }
            }
        }
    }

    private static boolean software(MediaCodec codec) {
        String name = codec.getName();
        return name.startsWith("c2.android.") || name.startsWith("OMX.google.");
    }

    private void pump(MediaCodec codec, int displayId, int dw, int dh, int rotation, boolean canRelay) throws IOException {
        MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
        long lastCheck = 0;
        long startedNs = System.nanoTime();
        boolean any = false;
        while (true) {
            if (lane.syncRequested) {
                lane.syncRequested = false;
                Bundle b = new Bundle();
                b.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0);
                codec.setParameters(b);
            }
            if (!lane.barriers.isEmpty()) {
                int ready;
                while ((ready = codec.dequeueOutputBuffer(info, 0)) >= 0) {
                    writeOutput(codec, ready, info);
                }
                long[] barrier;
                while ((barrier = lane.barriers.peek()) != null && encodedUpTo(barrier[1])) {
                    lane.barriers.poll();
                    out.writeLong(0);
                    out.writeByte(FLAG_BARRIER);
                    out.writeInt(8);
                    out.writeLong(barrier[0]);
                    out.flush();
                }
            }
            int id = codec.dequeueOutputBuffer(info, OUTPUT_WAIT_US);
            long now = System.nanoTime();
            if (id >= 0) {
                any = true;
            } else if (canRelay && !any && now - startedNs > NO_OUTPUT_NS) {
                // A new display always draws once, so an encoder that has given nothing by now gets no frames.
                relayFallback = true;
                System.err.println("video: no frame from the encoder in " + NO_OUTPUT_NS / 1_000_000
                        + " ms; trying again through an OpenGL relay");
                return;
            }
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

    // An idle screen gives the encoder no input, so a keyframe asked for then comes only when something moves. Asked
    // for at the first repeat after a change, it comes with the next repeat: the stream's last keyframe then shows
    // the still screen, and a recording started on it can open with that picture and nothing from before (D107).
    private void wrote(long ptsUs, boolean keyframe) {
        long now = System.nanoTime() / 1000;
        long lag = now - ptsUs;
        boolean timed = lag >= 0 && lag < CLOCK_LIMIT_US;
        lane.videoLagUs = timed ? lag : 0;
        boolean repeat = ptsUs - wrotePtsUs == REPEAT_US;
        if (!repeat) {
            unsettled = !keyframe;
        } else if (unsettled) {
            unsettled = false;
            lane.syncRequested = true;
        }
        wrotePtsUs = ptsUs;
        wroteAtUs = now;
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
                if ((flags & 1) == 0) {
                    wrote(info.presentationTimeUs, (flags & 2) != 0);
                }
            }
        } finally {
            codec.releaseOutputBuffer(id, false);
        }
    }
}
