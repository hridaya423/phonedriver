package dev.phonedriver.agent;

import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.Looper;
import android.os.SystemClock;
import com.genymobile.scrcpy.Workarounds;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.security.MessageDigest;
import org.json.JSONObject;

public final class Main {
    private static final long NOBODY_CONNECTED_EXIT_MS = 10_000;
    private static final int HELLO_MAX_BYTES = 1 << 20;
    /** Any app on the device can open the socket; one that never says hello held a thread until the host left. */
    private static final int HELLO_TIMEOUT_MS = 5_000;

    static String version;
    private static byte[] token;
    private static boolean everConnected;
    private static int streams;

    public static void main(String[] args) {
        try {
            Looper.prepareMainLooper();
            run(args);
        } catch (Throwable t) {
            t.printStackTrace();
            System.exit(1);
        }
    }

    private static String readLine(InputStream in) throws IOException {
        StringBuilder sb = new StringBuilder();
        int c;
        while ((c = in.read()) >= 0 && c != '\n') {
            sb.append((char) c);
        }
        return sb.toString();
    }

    private static void run(String[] args) throws Exception {
        version = args[0];
        String socketName = args[1];
        token = readLine(System.in).getBytes("UTF-8");
        if (token.length == 0) {
            System.err.println("no token on stdin");
            System.exit(2);
        }
        Workarounds.apply();
        try {
            Accessibility.connect();
        } catch (IllegalStateException e) {
            System.err.println("accessibility: " + e);
            System.exit(4);
        }
        Monitor.start();
        LocalServerSocket server = new LocalServerSocket(socketName);
        System.out.println("ready");
        System.out.flush();
        Thread watchdog = new Thread(() -> {
            SystemClock.sleep(NOBODY_CONNECTED_EXIT_MS);
            synchronized (Main.class) {
                if (!everConnected) {
                    System.err.println("nothing connected within 10 s, exiting");
                    System.exit(3);
                }
            }
        });
        watchdog.setDaemon(true);
        watchdog.start();
        while (true) {
            LocalSocket socket = server.accept();
            opened();
            new Thread(() -> serve(socket)).start();
        }
    }

    private static byte[] readFrame(DataInputStream in, int max) throws IOException {
        int n = in.readInt();
        if (n < 0 || n > max) {
            throw new IOException("frame length " + n);
        }
        byte[] b = new byte[n];
        in.readFully(b);
        return b;
    }

    private static boolean tokenMatches(JSONObject o) {
        return MessageDigest.isEqual(token, o.optString("token").getBytes());
    }

    private static synchronized void connected() {
        everConnected = true;
    }

    private static synchronized void opened() {
        streams++;
    }

    /** True when the stream that just closed was the last one of a host that had connected. */
    private static synchronized boolean closedTheLast() {
        streams--;
        return streams == 0 && everConnected;
    }

    private static void refuse(DataOutputStream out, JSONObject hello, String error) throws IOException {
        JSONObject reply = Json.put(new JSONObject(), "id", hello.optInt("id"));
        Json.put(reply, "ok", false);
        Json.put(reply, "error", error);
        byte[] frame = Json.bytes(reply);
        out.writeInt(frame.length);
        out.write(frame);
        out.flush();
    }

    private static void serve(LocalSocket socket) {
        int kind = 0;
        Lane lane = null;
        try {
            Looper.prepare();
            DataInputStream in = new DataInputStream(socket.getInputStream());
            DataOutputStream out = new DataOutputStream(
                    new java.io.BufferedOutputStream(socket.getOutputStream(), 1 << 16));
            socket.setSoTimeout(HELLO_TIMEOUT_MS);
            kind = in.read();
            if (kind != 'C' && kind != 'V') {
                System.err.println("unknown stream kind " + kind);
                return;
            }
            JSONObject first = Json.parse(readFrame(in, HELLO_MAX_BYTES));
            if (!tokenMatches(first)) {
                System.err.println("bad token, dropping");
                return;
            }
            socket.setSoTimeout(0);
            connected();
            boolean hidden = "hidden".equals(first.optString("display"));
            String name = first.optString("lane");
            if (kind == 'C') {
                try {
                    lane = Lane.claimControl(hidden, name);
                } catch (IllegalStateException e) {
                    System.err.println("control stream refused: " + e.getMessage());
                    refuse(out, first, e.getMessage());
                    return;
                }
            } else {
                lane = Lane.claimVideo(hidden, name);
                if (lane == null) {
                    System.err.println("video stream for a lane that is not open or has one, dropping");
                    return;
                }
            }
            lane.enter();
            if (kind == 'C') {
                new Control(in, out).run(first);
            } else {
                new Video(out, first.optInt("max_size", 0), first.optInt("bit_rate", 0), first.optInt("fps", 0)).run();
            }
        } catch (Throwable t) {
            System.err.println("stream " + (char) kind + " ended: " + t);
        } finally {
            try {
                socket.close();
            } catch (IOException ignored) {
            }
            if (lane != null) {
                Lane.release(lane, kind);
            }
            if (closedTheLast()) {
                System.err.println("every stream closed, exiting");
                System.exit(0);
            }
        }
    }
}
