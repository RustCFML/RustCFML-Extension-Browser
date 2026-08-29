<cfscript>
p = Browser().newPage().goto( "https://example.com/", { waitUntil = "load" } );

writeOutput( "title:    " & p.title() & chr(10) );
writeOutput( "h1 text:  " & p.text( "h1" ) & chr(10) );
writeOutput( "text():   " & left( p.text(), 40 ) & "..." & chr(10) );
writeOutput( "count(p): " & p.count( "p" ) & chr(10) );
writeOutput( "exists:   " & p.exists( "h1" ) & " / " & p.exists( ".nope" ) & chr(10) );
writeOutput( "attr:     " & p.attr( "a", "href" ) & chr(10) );
writeOutput( "links:    " & arrayLen( p.links() ) & " -> " & p.links()[1] & chr(10) );
writeOutput( "evaluate: " & p.evaluate( "document.querySelectorAll('p').length" ) & chr(10) );
writeOutput( "eval obj: " & serializeJSON( p.evaluate( "({a:1,b:[2,3],c:'x'})" ) ) & chr(10) );

png = p.screenshot( { width = 600, height = 400 } );
fileWrite( expandPath( "shot.png" ), png );
writeOutput( "png:      " & len( png ) & " bytes" & chr(10) );

pdf = p.pdf( { paper = "A4", printBackground = true } );
fileWrite( expandPath( "out.pdf" ), pdf );
writeOutput( "pdf:      " & len( pdf ) & " bytes, header=" & left( toString( pdf ), 5 ) & chr(10) );

p.close();
</cfscript>
