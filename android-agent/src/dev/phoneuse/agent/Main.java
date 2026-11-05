package dev.phoneuse.agent;

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

    static String version;
    private static byte[] token;
    private static boolean controlTaken;
    private static boolean videoTaken;
    private static boolean everConnected;

    static volatile Control control;

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

    private static synchronized boolean claim(int kind) {
        if (kind == 'C' && !controlTaken) {
            controlTaken = true;
        } else if (kind == 'V' && !videoTaken) {
            videoTaken = true;
        } else {
            return false;
        }
        everConnected = true;
        return true;
    }

    private static synchronized void release(int kind) {
        if (kind == 'C') {
            controlTaken = false;
        } else {
            videoTaken = false;
        }
        if (!controlTaken && !videoTaken) {
            System.err.println("both streams closed, exiting");
            System.exit(0);
        }
    }

    private static void serve(LocalSocket socket) {
        int kind = 0;
        boolean claimed = false;
        try {
            Looper.prepare();
            DataInputStream in = new DataInputStream(socket.getInputStream());
            DataOutputStream out = new DataOutputStream(
                    new java.io.BufferedOutputStream(socket.getOutputStream(), 1 << 16));
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
            if (!claim(kind)) {
                System.err.println("stream kind " + (char) kind + " already open, dropping");
                return;
            }
            claimed = true;
            if (kind == 'C') {
                new Control(in, out).run(first);
            } else {
                new Video(out, first.optInt("max_size", 0)).run();
            }
        } catch (Throwable t) {
            System.err.println("stream " + (char) kind + " ended: " + t);
        } finally {
            try {
                socket.close();
            } catch (IOException ignored) {
            }
            if (claimed) {
                release(kind);
            }
        }
    }
}
