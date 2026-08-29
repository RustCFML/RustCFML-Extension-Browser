<cfscript>
p = Browser().newPage().goto( "file://#getDirectoryFromPath( getCurrentTemplatePath() )#/app.html" );
p.waitForSelector( "##q", 5000 );                 // element arrives 400ms late
p.fill( "##q", "rustcfml" )
 .selectOption( "##s", "b" )
 .press( "Enter", "##q" )
 .click( "##go" )
 .settle( 300 );
writeOutput( "results: " & p.count( ".r" ) & chr(10) );
for ( row in p.extract( ".r" ) ) { writeOutput( "  - " & row.text & chr(10) ); }
writeOutput( "input value now: " & p.evaluate( "document.getElementById('q').value" ) & chr(10) );
p.close();
</cfscript>
