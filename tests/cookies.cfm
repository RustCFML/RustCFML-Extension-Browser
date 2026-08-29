<cfscript>
u = "https://rustcfml-worker.rustcfml.workers.dev/echo/request.cfm";
c = Browser();
c.setCookies( [ { name="sid", value="secret-A", domain="rustcfml-worker.rustcfml.workers.dev", path="/" } ] );
body = c.newPage().goto( u ).text();
writeOutput( "cookie reached the server: " & yesNoFormat( findNoCase( "secret-A", body ) gt 0 ) & chr(10) );
d = Browser();
body2 = d.newPage().goto( u ).text();
writeOutput( "fresh browser has none:    " & yesNoFormat( findNoCase( "secret-A", body2 ) eq 0 ) & chr(10) );
</cfscript>
