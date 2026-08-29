<cfscript>
p = Browser().newPage().goto( "file://#getDirectoryFromPath( getCurrentTemplatePath() )#/hist.html" ).settle( 300 );
writeOutput( "console messages:" & chr(10) );
for ( m in p.consoleMessages() ) { writeOutput( "   [" & m.level & "] " & m.text & chr(10) ); }
writeOutput( "title now: " & p.title() & chr(10) );
p.goto( "file://#getDirectoryFromPath( getCurrentTemplatePath() )#/two.html" );
writeOutput( "after goto: " & p.title() & chr(10) );
p.back();
writeOutput( "after back: " & p.title() & chr(10) );
p.forward();
writeOutput( "after fwd:  " & p.title() & chr(10) );
p.reload();
writeOutput( "after reload: " & p.title() & chr(10) );
p.close();
</cfscript>
