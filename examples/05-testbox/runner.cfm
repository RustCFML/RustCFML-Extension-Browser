<cfscript>
if ( !directoryExists( expandPath( "/testbox/system" ) ) ) {
    writeOutput( "TestBox is not where this example expects it.<br>Run <code>box install testbox</code> in this directory, or edit the mapping in Application.cfc." );
    abort;
}
// ?reporter=text for a terminal or CI; the default HTML reporter in a browser.
reporter = url.reporter ?: "simple";
if ( reporter eq "text" ) { getPageContext().getResponse().setContentType( "text/plain" ); }

writeOutput(
    new testbox.system.TestBox( directory = { recurse = true, mapping = "specs" } )
        .run( reporter = reporter )
);
</cfscript>
