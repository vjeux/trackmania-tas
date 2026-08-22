/* Main server class */


enum ServerState {
    NotStarted,
    Running,
    Shutdown,
    Error
}

class HttpResponse {
    int status = 405;
    string _body;
    MemoryBuffer@ _buf;
    dictionary headers;

    string body {
        get { return _body; }
    }

    // yep works
    void set_body(const string &in value) {
        _body = value;
        headers['Content-Length'] = tostring(value.Length);
    }

    HttpResponse() {
        InitHeaders(0);
    }
    HttpResponse(int status, const string &in body = "") {
        InitHeaders(body.Length);
        this.status = status;
        this.body = body;
    }
    HttpResponse(int status, MemoryBuffer@ buf) {
        buf.Seek(0);
        InitHeaders(buf.GetSize(), "application/octet-stream");
        this.status = status;
        @this._buf = buf;
    }

    protected void InitHeaders(uint contentLength, const string &in contentType = "text/plain") {
        headers['Content-Length'] = tostring(contentLength);
        headers['Content-Type'] = contentType;
        headers['Server'] = "AngelScript HttpServer " + Meta::ExecutingPlugin().Version;
        headers['Connection'] = "close";
    }

    const string StatusMsgText() {
        switch (status) {
            case 200: return "OK";
            case 404: return "Not Found";
            case 405: return "Method Not Allowed";
            case 500: return "Internal Server Error";
        }
        if (status < 300) return "OK?";
        if (status < 400) return "Redirect?";
        if (status < 500) return "Request Error?";
        return "Server Error?";
    }
}

// Returns status
funcdef HttpResponse@ ReqHandlerFunc(const string &in type, const string &in route, dictionary@ headers, MemoryBuffer@ body);

/* An http server. Call `.StartServer()` to start listening. Default port is 29805 and default host is localhost. */
class HttpServer {
    // 29805 = 0x746d = 'tm'
    uint16 port = 29805;
    string host = "localhost";
    protected ServerState state = ServerState::NotStarted;

    HttpServer() {}
    HttpServer(uint16 port) {
        this.port = port;
    }
    HttpServer(const string &in hostname) {
        this.host = hostname;
    }
    HttpServer(const string &in hostname, uint16 port) {
        this.port = port;
        this.host = hostname;
    }

    protected Net::Socket@ socket = null;
    ReqHandlerFunc@ RequestHandler = null;

    void Shutdown() {
        state = ServerState::Shutdown;
        try {
            socket.Close();
        } catch {}
        trace("Server shut down.");
    }

    void StartServer() {
        if (RequestHandler is null) {
            throw("Must set .RequestHandler before starting server!");
        }
        if (state != ServerState::NotStarted) {
            throw("Cannot start HTTP server twice.");
        }
        @socket = Net::Socket();
        trace("Starting server: " + host + ":" + port);
        if (!socket.Listen(host, port)) {
            SetError("failed to start listening");
            return;
        }
        state = ServerState::Running;
        trace("Server running.");
        startnew(CoroutineFunc(this.AcceptConnections));
    }

    protected void SetError(const string &in errMsg) {
        warn('HttpServer terminated with error: ' + errMsg);
        state = ServerState::Error;
        try {
            socket.Close();
        } catch {};
        @socket = null;
    }

    protected void AcceptConnections() {
        while (state == ServerState::Running) {
            yield();
            auto client = socket.Accept();
            if (client is null) continue;
            trace("Accepted new client // Remote: " + client.GetRemoteIP());
            startnew(CoroutineFuncUserdata(this.RunClient), client);
        }
    }

    protected void RunClient(ref@ clientRef) {
        auto client = cast<Net::Socket>(clientRef);
        if (client is null) return;
        uint clientStarted = Time::Now;
        while (Time::Now - clientStarted < 10000 && client.Available() == 0) yield();
        if (client.Available() == 0) {
            trace("Timing out client: " + client.GetRemoteIP());
            client.Close();
            return;
        }
        RunRequest(client);
        trace("Closing client.");
        client.Close();
    }

    protected void RunRequest(Net::Socket@ client) {
        string reqLine;
        if (!client.ReadLine(reqLine)) {
            warn("RunRequest: could not read first line!");
            return;
        }
        reqLine = reqLine.Trim();
        auto reqParts = reqLine.Split(" ", 3);
        trace("RunRequest got first line: " + reqLine + " (parts: " + reqParts.Length + ")");
        auto headers = ParseHeaders(client);
        trace("Got " + headers.GetSize() + " headers.");
        // auto headerKeys = headers.GetKeys();
        auto reqType = reqParts[0];
        auto reqRoute = reqParts[1];
        auto httpVersion = reqParts[2];
        if (!httpVersion.StartsWith("HTTP/1.")) {
            warn("Unsupported HTTP version: " + httpVersion);
            return;
        }
        MemoryBuffer@ buf;
        if (headers.Exists('Content-Length')) {
            auto len = Text::ParseInt(string(headers['Content-Length']));
            // data = client.ReadRaw(len);
            @buf = client.ReadBuffer(len);
        }
        if (client.Available() > 0) {
            warn("After reading headers and body there are " + client.Available() + " bytes remaining!");
        }
        HttpResponse@ resp = HttpResponse();
        try {
            @resp = RequestHandler(reqType, reqRoute, headers, buf);
        } catch {
            error("Exception in RequestHandler: " + getExceptionInfo());
            resp.status = 500;
            resp.body = "Exception: " + getExceptionInfo();
        }
        string respHdrsStr = FormatHeaders(resp.headers);
        string fullResponse = httpVersion + " " + resp.status + " " + resp.StatusMsgText() + "\r\n" + respHdrsStr;
        fullResponse += "\r\n\r\n" + resp.body;
        auto respBuf = MemoryBuffer();
        respBuf.Write(fullResponse);
        trace("Response: " + fullResponse);
        if (resp._buf !is null) {
            resp._buf.Seek(0);
            respBuf.WriteFromBuffer(resp._buf, resp._buf.GetSize());
        }
        // need to use WriteRaw b/c otherwise strings are length prefixed
        // client.WriteRaw(fullResponse);
        respBuf.Seek(0);
        client.Write(respBuf, respBuf.GetSize());
        trace("["+Time::Stamp + " | " + client.GetRemoteIP()+"] " + reqType + " " + reqRoute + " " + resp.status);
        trace("Completed request.");
    }

    protected dictionary@ ParseHeaders(Net::Socket@ client) {
        dictionary headers;
        string nextLine;
        while (true) {
            while (client.Available() == 0) yield();
            client.ReadLine(nextLine);
            nextLine = nextLine.Trim();
            if (nextLine.Length > 0) {
                AddHeader(headers, nextLine);
            } else break;
        }
        return headers;
    }

    protected void AddHeader(dictionary@ d, const string &in line) {
        auto parts = line.Split(":", 2);
        if (parts.Length < 2) {
            warn("Header line failed to parse: " + line + " // " + parts[0]);
        } else {
            d[parts[0]] = parts[1];
            if (parts[0].ToLower().Contains("authorization")) {
                parts[1] = "<auth omitted>";
            }
            trace("Parsed header line: " + parts[0] + ": " + parts[1]);
        }
    }
}


string FormatHeaders(dictionary@ headers) {
    auto keys = headers.GetKeys();
    for (uint i = 0; i < keys.Length; i++) {
        if (keys[i].ToLower().Contains("authorization")) {
            keys[i] += ": <auth omitted>";
        } else {
            keys[i] += ": " + string(headers[keys[i]]);
        }
    }
    return string::Join(keys, "\r\n");
}


HttpResponse@ _404_Response = HttpResponse(404, "Not found");
