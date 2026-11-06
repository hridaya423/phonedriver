package dev.phoneuse.agent;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import org.json.JSONObject;

final class Shell {
    private static final int CAP_BYTES = 1 << 20;

    static JSONObject run(String command, long timeoutMs) throws Exception {
        Process p = new ProcessBuilder("sh", "-c", command).start();
        p.getOutputStream().close();
        Drain out = new Drain(p.getInputStream());
        Drain err = new Drain(p.getErrorStream());
        out.start();
        err.start();
        long deadline = System.currentTimeMillis() + timeoutMs;
        Integer exit = null;
        while (exit == null) {
            try {
                exit = p.exitValue();
            } catch (IllegalThreadStateException running) {
                if (System.currentTimeMillis() >= deadline) {
                    break;
                }
                Thread.sleep(2);
            }
        }
        boolean timedOut = exit == null;
        if (timedOut) {
            p.destroy();
            exit = 124;
        }
        out.join(500);
        err.join(500);
        JSONObject r = new JSONObject();
        Json.put(r, "exit", exit);
        Json.put(r, "stdout", out.text());
        Json.put(r, "stderr", err.text());
        Json.put(r, "timed_out", timedOut);
        return r;
    }

    private static final class Drain extends Thread {
        private final InputStream in;
        private final ByteArrayOutputStream buf = new ByteArrayOutputStream();

        Drain(InputStream in) {
            this.in = in;
        }

        @Override
        public void run() {
            byte[] chunk = new byte[8192];
            try {
                int n;
                while ((n = in.read(chunk)) > 0) {
                    synchronized (buf) {
                        int room = CAP_BYTES - buf.size();
                        if (room > 0) {
                            buf.write(chunk, 0, Math.min(n, room));
                        }
                    }
                }
            } catch (IOException ended) {
            }
        }

        String text() throws java.io.UnsupportedEncodingException {
            synchronized (buf) {
                return buf.toString("UTF-8");
            }
        }
    }

    private Shell() {}
}
