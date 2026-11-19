package dev.daijro.honeydew;

import java.util.ArrayDeque;
import java.util.concurrent.TimeUnit;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;

/** An Activity-owned transport. Callbacks never mutate the Rust project; the app drains them between frames. */
final class HoneydewRooms {
    private static final OkHttpClient HTTP = new OkHttpClient.Builder()
            .pingInterval(5, TimeUnit.SECONDS).followRedirects(false).followSslRedirects(false).build();
    private static final int MAX_MESSAGE = 16 * 1024 * 1024 + 2048;
    private static final int MAX_QUEUE = 2 * MAX_MESSAGE;
    private final ArrayDeque<String> inbox = new ArrayDeque<>();
    private int queued;
    private WebSocket socket;
    private boolean resumed;

    synchronized boolean open(String endpoint, String route, String name, String token) {
        if (!resumed) return false;
        HttpUrl base = HttpUrl.parse(endpoint);
        if (base == null || !base.isHttps() || !base.username().isEmpty() || !base.password().isEmpty()
                || base.query() != null || base.fragment() != null || !base.encodedPath().equals("/")) return false;
        close();
        inbox.clear();
        queued = 0;
        HttpUrl url = base.newBuilder().addPathSegment("rooms").addPathSegment(route)
                .addQueryParameter("name", name).build();
        Request.Builder request = new Request.Builder().url(url);
        if (!token.isEmpty()) request.header("Authorization", "Bearer " + token);
        socket = HTTP.newWebSocket(request.build(), new WebSocketListener() {
            @Override public void onMessage(WebSocket ws, String text) {
                synchronized (HoneydewRooms.this) {
                    if (socket != ws) return;
                    if (text.length() > MAX_MESSAGE || queued + text.length() > MAX_QUEUE || inbox.size() >= 256) {
                        ws.cancel();
                        socket = null;
                        enqueue("{\"type\":\"closed\",\"status\":0}");
                        return;
                    }
                    enqueue(text);
                }
            }
            @Override public void onClosing(WebSocket ws, int code, String reason) {
                ws.close(1000, null);
                closed(ws, 0);
            }
            @Override public void onFailure(WebSocket ws, Throwable error, Response response) {
                closed(ws, response == null ? 0 : response.code());
            }
        });
        return true;
    }

    private synchronized void closed(WebSocket ws, int status) {
        if (socket != ws) return;
        socket = null;
        enqueue("{\"type\":\"closed\",\"status\":" + status + "}");
    }

    private void enqueue(String text) { inbox.addLast(text); queued += text.length(); }

    synchronized String poll() {
        String text = inbox.pollFirst();
        if (text != null) queued -= text.length();
        return text;
    }

    synchronized boolean send(String message) { return socket != null && socket.send(message); }

    synchronized void close() {
        WebSocket old = socket;
        socket = null;
        if (old != null) old.close(1000, null);
    }

    synchronized void foreground(boolean value) {
        resumed = value;
        if (!value && socket != null) {
            close();
            enqueue("{\"type\":\"closed\",\"status\":0}");
        }
    }
}
