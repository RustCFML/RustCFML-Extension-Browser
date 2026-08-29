<cfscript>
p = Browser().newPage();
p.mock( "*/api/flags*", { status = 200, body = '{"feature":"MOCKED"}' } );
p.goto( "http://127.0.0.1:8794/" ).settle( 2000 );
out = p.text( "##out" );
writeOutput( "out: " & out & chr(10) );
writeOutput( "mock applied:            " & yesNoFormat( findNoCase("feature","")eq 0 && findNoCase("mocked=MOCKED", out) gt 0 ) & chr(10) );
writeOutput( "unmatched still reached server: " & yesNoFormat( findNoCase("real=live-from-server", out) gt 0 ) & chr(10) );
p.close();
</cfscript>
