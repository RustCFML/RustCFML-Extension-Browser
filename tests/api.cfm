<cfscript>
fails = 0;
function ok( label, expr ) {
  try { var v = expr(); writeOutput( "  ok    " & label & chr(10) ); return v; }
  catch (any e) { writeOutput( "  FAIL  " & label & " -> " & e.message & chr(10) ); request.f = 1; }
}
b = Browser( { timeout = 30000 } );
ok( "browserVersion()", function(){ return browserVersion(); } );
ok( "isBrowserObject(b)", function(){ if ( !isBrowserObject(b) ) throw("false"); return 1; } );
ok( "browser.cookies()", function(){ return b.cookies(); } );
ok( "browser.setCookies([])", function(){ return b.setCookies([]); } );
ok( "browser.clearCookies()", function(){ return b.clearCookies(); } );
p = ok( "browser.newPage()", function(){ return b.newPage(); } );
ok( "goto", function(){ return p.goto( "https://example.com/", { waitUntil="load" } ); } );
ok( "waitForSelector", function(){ return p.waitForSelector( "h1", 5000 ); } );
ok( "waitForText", function(){ return p.waitForText( "Example", 5000 ); } );
ok( "settle", function(){ return p.settle( 200 ); } );
ok( "setViewport", function(){ return p.setViewport( 1000, 700 ); } );
ok( "url/title/content", function(){ return p.url() & p.title() & len(p.content()); } );
ok( "text()", function(){ return p.text(); } );
ok( "text(sel)", function(){ return p.text("h1"); } );
ok( "markdown()", function(){ return p.markdown(); } );
ok( "markdown(sel)", function(){ return p.markdown("h1"); } );
ok( "links()", function(){ return p.links(); } );
ok( "attr", function(){ return p.attr("a","href"); } );
ok( "count", function(){ return p.count("p"); } );
ok( "exists", function(){ return p.exists("h1"); } );
ok( "extract", function(){ return p.extract("p"); } );
ok( "extractQuery", function(){ var q = p.extractQuery("a"); if ( !isQuery(q) ) throw("not a query"); return q; } );
ok( "boundingBox", function(){ return p.boundingBox("h1"); } );
ok( "evaluate", function(){ return p.evaluate("1+1"); } );
ok( "requests", function(){ return p.requests(); } );
ok( "consoleMessages", function(){ return p.consoleMessages(); } );
ok( "reload", function(){ return p.reload(); } );
ok( "back (no history -> errors)", function(){ try { p.back(); throw("should have failed"); } catch(any e){ if ( findNoCase("no earlier page", e.message) ) return "errored correctly"; rethrow; } } );
ok( "fonts", function(){ return p.fonts(); } );
ok( "block", function(){ return p.block(["*.png*"]); } );
ok( "mock", function(){ return p.mock( "*/api/x*", { status=200, body='{}' } ); } );
ok( "screenshot", function(){ return p.screenshot({width:400,height:300}); } );
ok( "screenshot detail", function(){ return p.screenshot({detail:true,width:400,height:300}); } );
ok( "pdf", function(){ return p.pdf({paper:"A4"}); } );
ok( "click", function(){ return p.click("a"); } );
ok( "fill (no input -> should error)", function(){ try { p.fill("##nope","x"); throw("should have failed"); } catch(any e){ if ( findNoCase("no element", e.message) ) return "errored correctly"; rethrow; } } );
ok( "close", function(){ return p.close(); } );
ok( "browser.close", function(){ return b.close(); } );
ok( "browserServer + honest stop", function(){
  var s = browserServer( "cdp", { port = 9231 } );
  if ( s.port() neq 9231 ) throw( "wrong port" );
  try { s.stop(); return "stopped"; } catch (any e) {
    if ( findNoCase( "did not shut down", e.message ) ) return "reported honestly";
    rethrow;
  }
} );
writeOutput( chr(10) & ( structKeyExists(request,"f") ? "SOME FAILED" : "ALL DOCUMENTED METHODS OK" ) & chr(10) );
</cfscript>
