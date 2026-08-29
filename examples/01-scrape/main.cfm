<cfscript>
/*  Scrape a page into CFML data.

    No Chrome, no Selenium, no JVM — the browser runs inside this process.
    JavaScript executes, so this works on pages that build themselves client
    side, not just static HTML.
*/

page = Browser().newPage().goto( "https://news.ycombinator.com/" );

writeOutput( "Title: " & page.title() & chr(10) );
writeOutput( "Links on the page: " & arrayLen( page.links() ) & chr(10) & chr(10) );

// extract() returns one struct per match: its text, plus every attribute.
stories = page.extract( ".titleline > a" );

writeOutput( "Top 5 stories" & chr(10) );
writeOutput( repeatString( "-", 60 ) & chr(10) );
for ( i = 1; i <= min( 5, arrayLen( stories ) ); i++ ) {
    writeOutput( i & ". " & stories[ i ].text & chr(10) );
    writeOutput( "   " & stories[ i ].href & chr(10) );
}

// Anything the DOM can answer, evaluate() can return as real CFML values.
counts = page.evaluate( "
    ({ stories: document.querySelectorAll('.titleline').length,
       comments: document.querySelectorAll('a[href^=""item?id=""]').length })
" );
writeOutput( chr(10) & "evaluate(): " & serializeJSON( counts ) & chr(10) );

// Or take the whole page as markdown, ready to hand to an LLM.
md = page.markdown();
writeOutput( "markdown(): " & len( md ) & " chars, first line: "
             & listFirst( trim( md ), chr(10) ) & chr(10) );

page.close();
</cfscript>
