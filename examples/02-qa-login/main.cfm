<cfscript>
/*  QA a login flow, then prove it worked with a screenshot.

    login.cfm sits beside this file, so it is served from the same host and
    path as this page. Its dashboard is built by JavaScript after the click —
    nothing here is visible in the raw HTML. Against your own app, just swap
    the URL.
*/

here  = getDirectoryFromPath( getCurrentTemplatePath() );
base  = ( cgi.https == "on" ? "https" : "http" ) & "://" & cgi.http_host & getDirectoryFromPath( cgi.script_name );
try {
    page = Browser().newPage().goto( base & "login.cfm" );
} catch ( any e ) {
    // The guard refuses private addresses unless you opt in. Driving your own
    // app is precisely when you mean to, so say so rather than turning the
    // guard off in code.
    if ( e.message contains "private/internal" ) {
        writeOutput( "This example drives the browser at this very server, and the renderer" & chr(10)
                   & "refuses loopback by default. Restart with the opt-in:" & chr(10) & chr(10)
                   & "    OBSCURA_ALLOW_PRIVATE_NETWORK=1 rustcfml --serve" & chr(10) );
        abort;
    }
    rethrow;
}

// 1. The wrong password must show the error and must NOT log us in.
page.fill( "##username", "sysadmin" )
    .fill( "##password", "wrong" )
    .click( "##loginButton" )
    .settle( 200 );

writeOutput( "wrong password -> error shown:  " & yesNoFormat( page.evaluate( "getComputedStyle(document.getElementById('error')).display !== 'none'" ) ) & chr(10) );
writeOutput( "wrong password -> dashboard:    " & yesNoFormat( page.exists( ".dashboard" ) ) & chr(10) );

// 2. The right password gets us to the dashboard. waitForSelector blocks until
//    the JS has actually built it (or throws after 5s).
page.fill( "##password", "password" )
    .click( "##loginButton" )
    .waitForSelector( ".dashboard", 5000 );

writeOutput( "right password -> heading:      " & page.text( ".dashboard h1" ) & chr(10) );
writeOutput( "right password -> KPIs:         " & page.count( ".kpi" ) & chr(10) );

// 3. Evidence. `detail=true` tells you whether anything was actually painted —
//    a blank PNG is what a failed hydration looks like, and it is still a valid PNG.
shot = page.screenshot( { width = 1024, height = 600, detail = true } );
if ( shot.looksUnrendered ) {
    throw( "dashboard painted nothing — did the page hydrate?" );
}
fileWrite( "#here#login-worked.png", shot.image );
writeOutput( "screenshot:                     login-worked.png (" & numberFormat( shot.inkCoverage * 100, "0.0" ) & "% ink, " & shot.distinctColours & " colours)" & chr(10) );

page.close();
</cfscript>
