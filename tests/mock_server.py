import http.server, socketserver
PAGE = b"""<!doctype html><html><body><div id="out">waiting</div><script>
var log=[];
function render(){document.getElementById('out').textContent=log.join(' | ');}
fetch('/api/flags').then(r=>r.json()).then(j=>{log.push('mocked='+j.feature);render();}).catch(e=>{log.push('mock-FAILED');render();});
fetch('/api/real').then(r=>r.text()).then(t=>{log.push('real='+t);render();}).catch(e=>{log.push('real-FAILED:'+e);render();});
</script></body></html>"""
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/api/flags": body, ct = b'{"feature":"SERVER"}', "application/json"
        elif self.path == "/api/real": body, ct = b"live-from-server", "text/plain"
        else: body, ct = PAGE, "text/html"
        self.send_response(200); self.send_header("Content-Type", ct)
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self,*a): pass
socketserver.TCPServer(("127.0.0.1", 8794), H).serve_forever()
