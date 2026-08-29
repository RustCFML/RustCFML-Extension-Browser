<cfscript>
// A page that renders, and one whose script never fills the shell.
fileWrite( "#getDirectoryFromPath( getCurrentTemplatePath() )#/broken.html", '<!doctype html><html><body><div id="root"></div><script>/* hydration never happens */</script></body></html>' );

good = Browser().newPage().goto( "https://example.com/" ).screenshot( { width=600, height=400, detail=true } );
writeOutput( "good:   coverage=" & numberFormat( good.inkCoverage*100, "0.00" ) & "%  colours=" & good.distinctColours & "  unrendered=" & good.looksUnrendered & chr(10) );

bad = Browser().newPage().goto( "file://#getDirectoryFromPath( getCurrentTemplatePath() )#/broken.html" ).screenshot( { width=600, height=400, detail=true } );
writeOutput( "broken: coverage=" & numberFormat( bad.inkCoverage*100, "0.00" ) & "%  colours=" & bad.distinctColours & "  unrendered=" & bad.looksUnrendered & chr(10) );

// full page vs viewport
vp = Browser().newPage().goto( "https://en.wikipedia.org/wiki/Rust_(programming_language)" );
a = vp.screenshot( { width=1000, height=600 } );
b = vp.screenshot( { width=1000, height=600, fullPage=true } );
writeOutput( "viewport=" & len(a) & " bytes, fullPage=" & len(b) & " bytes" & chr(10) );
vp.close();
</cfscript>
