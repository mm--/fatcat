
"""
Helpers for doing elasticsearch queries (used in the web interface; not part of
the formal API)
"""

import sys
import datetime
from dataclasses import dataclass
from typing import List, Optional, Any

import elasticsearch
from elasticsearch_dsl import Search, Q
import elasticsearch_dsl.response

from fatcat_web import app

class FatcatSearchError(Exception):

    def __init__(self, status_code: int, name: str, description: str = None):
        if status_code == "N/A":
            status_code = 503
        self.status_code = status_code
        self.name = name
        self.description = description

@dataclass
class ReleaseQuery:
    q: Optional[str] = None
    limit: Optional[int] = None
    offset: Optional[int] = None
    fulltext_only: bool = False
    container_id: Optional[str] = None
    recent: bool = False

    @classmethod
    def from_args(cls, args) -> 'ReleaseQuery':

        query_str = args.get('q') or '*'

        container_id = args.get('container_id')
        # TODO: as filter, not in query string
        if container_id:
            query_str += ' container_id:"{}"'.format(container_id)

        # TODO: where are container_issnl queries actually used?
        issnl = args.get('container_issnl')
        if issnl and query_str:
            query_str += ' container_issnl:"{}"'.format(issnl)

        offset = args.get('offset', '0')
        offset = max(0, int(offset)) if offset.isnumeric() else 0

        return ReleaseQuery(
            q=query_str,
            offset=offset,
            fulltext_only=bool(args.get('fulltext_only')),
            container_id=container_id,
            recent=bool(args.get('recent')),
        )

@dataclass
class GenericQuery:
    q: Optional[str] = None
    limit: Optional[int] = None
    offset: Optional[int] = None

    @classmethod
    def from_args(cls, args) -> 'GenericQuery':
        query_str = args.get('q')
        if not query_str:
            query_str = '*'
        offset = args.get('offset', '0')
        offset = max(0, int(offset)) if offset.isnumeric() else 0

        return GenericQuery(
            q=query_str,
            offset=offset,
        )

@dataclass
class SearchHits:
    count_returned: int
    count_found: int
    offset: int
    limit: int
    deep_page_limit: int
    query_time_ms: int
    results: List[Any]


def results_to_dict(response: elasticsearch_dsl.response.Response) -> List[dict]:
    """
    Takes a response returns all the hits as JSON objects.

    Also handles surrogate strings that elasticsearch returns sometimes,
    probably due to mangled data processing in some pipeline. "Crimes against
    Unicode"; production workaround
    """

    results = []
    for h in response:
        r = h._d_
        # print(h.meta._d_)
        results.append(r)

    for h in results:
        for key in h:
            if type(h[key]) is str:
                h[key] = h[key].encode("utf8", "ignore").decode("utf8")
    return results

def wrap_es_execution(search: Search) -> Any:
    """
    Executes a Search object, and converts various ES error types into
    something we can pretty print to the user.
    """
    try:
        resp = search.execute()
    except elasticsearch.exceptions.RequestError as e:
        # this is a "user" error
        print("elasticsearch 400: " + str(e.info), file=sys.stderr)
        description = None
        if e.info.get("error", {}).get("root_cause", {}):
            description = str(e.info["error"]["root_cause"][0].get("reason"))
        raise FatcatSearchError(e.status_code, str(e.error), description)
    except elasticsearch.exceptions.ConnectionError as e:
        raise FatcatSearchError(e.status_code, "ConnectionError: search engine not available")
    except elasticsearch.exceptions.TransportError as e:
        # all other errors
        print("elasticsearch non-200 status code: {}".format(e.info), file=sys.stderr)
        description = None
        if e.info.get("error", {}).get("root_cause", {}):
            description = str(e.info["error"]["root_cause"][0].get("reason"))
        raise FatcatSearchError(e.status_code, str(e.error), description)
    return resp

def agg_to_dict(agg) -> dict:
    """
    Takes a simple term aggregation result (with buckets) and returns a simple
    dict with keys as terms and counts as values. Includes an extra value
    '_other', and by convention aggregations should be writen to have "missing"
    vaules as '_unknown'.
    """
    result = dict()
    for bucket in agg.buckets:
        result[bucket.key] = bucket.doc_count
    if agg.sum_other_doc_count:
        result['_other'] = agg.sum_other_doc_count
    return result

def do_container_search(
    query: GenericQuery, deep_page_limit: int = 2000
) -> SearchHits:

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_CONTAINER_INDEX'])

    search = search.query(
        "query_string",
        query=query.q,
        default_operator="AND",
        analyze_wildcard=True,
        allow_leading_wildcard=False,
        lenient=True,
        fields=["biblio"],
    )

    # Sanity checks
    limit = min((int(query.limit or 25), 100))
    offset = max((int(query.offset or 0), 0))
    if offset > deep_page_limit:
        # Avoid deep paging problem.
        offset = deep_page_limit

    search = search[offset : (offset + limit)]

    resp = wrap_es_execution(search)
    results = results_to_dict(resp)

    return SearchHits(
        count_returned=len(results),
        count_found=int(resp.hits.total),
        offset=offset,
        limit=limit,
        deep_page_limit=deep_page_limit,
        query_time_ms=int(resp.took),
        results=results,
    )

def do_release_search(
    query: ReleaseQuery, deep_page_limit: int = 2000
) -> SearchHits:

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])

    # availability filters
    if query.fulltext_only:
        search = search.filter("term", in_ia=True)

    # Below, we combine several queries to improve scoring.

    # this query use the fancy built-in query string parser
    basic_biblio = Q(
        "query_string",
        query=query.q,
        default_operator="AND",
        analyze_wildcard=True,
        allow_leading_wildcard=False,
        lenient=True,
        fields=[
            "title^2",
            "biblio",
        ],
    )
    has_fulltext = Q("term", in_ia=True)
    poor_metadata = Q(
        "bool",
        should=[
            # if these fields aren't set, metadata is poor. The more that do
            # not exist, the stronger the signal.
            Q("bool", must_not=Q("exists", field="title")),
            Q("bool", must_not=Q("exists", field="release_year")),
            Q("bool", must_not=Q("exists", field="release_type")),
            Q("bool", must_not=Q("exists", field="release_stage")),
            Q("bool", must_not=Q("exists", field="container_id")),
        ],
    )

    search = search.query(
        "boosting",
        positive=Q("bool", must=basic_biblio, should=[has_fulltext],),
        negative=poor_metadata,
        negative_boost=0.5,
    )

    # Sanity checks
    limit = min((int(query.limit or 25), 100))
    offset = max((int(query.offset or 0), 0))
    if offset > deep_page_limit:
        # Avoid deep paging problem.
        offset = deep_page_limit

    search = search[offset : (offset + limit)]

    resp = wrap_es_execution(search)
    results = results_to_dict(resp)

    for h in results:
        # Ensure 'contrib_names' is a list, not a single string
        if type(h['contrib_names']) is not list:
            h['contrib_names'] = [h['contrib_names'], ]
        h['contrib_names'] = [name.encode('utf8', 'ignore').decode('utf8') for name in h['contrib_names']]

    return SearchHits(
        count_returned=len(results),
        count_found=int(resp.hits.total),
        offset=offset,
        limit=limit,
        deep_page_limit=deep_page_limit,
        query_time_ms=int(resp.took),
        results=results,
    )

def get_elastic_container_random_releases(ident: str, limit=5) -> dict:
    """
    Returns a list of releases from the container.
    """

    assert limit > 0 and limit <= 100

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search = search.query(
        'bool',
        must=[
            Q('term', container_id=ident),
            Q('range', release_year={ "lte": datetime.datetime.today().year }),
        ]
    )
    search = search.sort('-in_web', '-release_date')
    search = search[:int(limit)]

    search = search.params(request_cache=True)
    resp = wrap_es_execution(search)
    results = results_to_dict(resp)

    return results

def get_elastic_entity_stats() -> dict:
    """
    TODO: files, filesets, webcaptures (no schema yet)

    Returns dict:
        changelog: {latest: {index, datetime}}
        release: {total, refs_total}
        papers: {total, in_web, in_oa, in_kbart, in_web_not_kbart}
    """

    stats = {}

    # release totals
    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search.aggs.bucket(
        'release_ref_count',
        'sum',
        field='ref_count',
    )
    search = search[:0]  # pylint: disable=unsubscriptable-object

    search = search.params(request_cache=True)
    resp = wrap_es_execution(search)

    stats['release'] = {
        "total": int(resp.hits.total),
        "refs_total": int(resp.aggregations.release_ref_count.value),
    }

    # paper counts
    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search = search.query(
        'terms',
        release_type=[
            "article-journal",
            "paper-conference",
            # "chapter",
            # "thesis",
        ],
    )
    search.aggs.bucket(
        'paper_like',
        'filters',
        filters={
            "in_web": { "term": { "in_web": "true" } },
            "is_oa": { "term": { "is_oa": "true" } },
            "in_kbart": { "term": { "in_kbart": "true" } },
            "in_web_not_kbart": { "bool": { "filter": [
                { "term": { "in_web": "true" } },
                { "term": { "in_kbart": "false" } },
            ]}},
        }
    )
    search = search[:0]

    search = search.params(request_cache=True)
    resp = wrap_es_execution(search)
    buckets = resp.aggregations.paper_like.buckets
    stats['papers'] = {
        'total': resp.hits.total,
        'in_web': buckets.in_web.doc_count,
        'is_oa': buckets.is_oa.doc_count,
        'in_kbart': buckets.in_kbart.doc_count,
        'in_web_not_kbart': buckets.in_web_not_kbart.doc_count,
    }

    # container counts
    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_CONTAINER_INDEX'])
    search.aggs.bucket(
        'release_ref_count',
        'sum',
        field='ref_count',
    )
    search = search[:0]  # pylint: disable=unsubscriptable-object

    search = search.params(request_cache=True)
    resp = wrap_es_execution(search)
    stats['container'] = {
        "total": resp.hits.total,
    }

    return stats

def get_elastic_search_coverage(query: ReleaseQuery) -> dict:

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search = search.query(
        "query_string",
        query=query.q,
        default_operator="AND",
        analyze_wildcard=True,
        allow_leading_wildcard=False,
        lenient=True,
        fields=["biblio"],
    )
    search.aggs.bucket(
        'preservation',
        'terms',
        field='preservation',
        missing='_unknown',
    )
    if query.recent:
        date_today = datetime.date.today()
        start_date = str(date_today - datetime.timedelta(days=60))
        end_date = str(date_today + datetime.timedelta(days=1))
        search = search.filter("range", release_date=dict(gte=start_date, lte=end_date))

    search = search[:0]

    search = search.params(request_cache=True)
    resp = wrap_es_execution(search)

    preservation_bucket = agg_to_dict(resp.aggregations.preservation)
    preservation_bucket['total'] = resp.hits.total
    for k in ('bright', 'dark', 'shadows_only', 'none'):
        if not k in preservation_bucket:
            preservation_bucket[k] = 0
    stats = {
        'total': resp.hits.total,
        'preservation': preservation_bucket,
    }

    return stats

def get_elastic_container_stats(ident, issnl=None):
    """
    Returns dict:
        ident
        issnl (optional)
        total
        in_web
        in_kbart
        preserved
    """

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search = search.query(
        'term',
        container_id=ident,
    )
    search.aggs.bucket(
        'container_stats',
        'filters',
        filters={
            "in_web": {
                "term": { "in_web": True },
            },
            "in_kbart": {
                "term": { "in_kbart": True },
            },
            "is_preserved": {
                "term": { "is_preserved": True },
            },
        },
    )
    search.aggs.bucket(
        'preservation',
        'terms',
        field='preservation',
        missing='_unknown',
    )
    search.aggs.bucket(
        'release_type',
        'terms',
        field='release_type',
        missing='_unknown',
    )

    search = search[:0]

    search = search.params(request_cache=True)
    resp = wrap_es_execution(search)

    container_stats = resp.aggregations.container_stats.buckets
    preservation_bucket = agg_to_dict(resp.aggregations.preservation)
    preservation_bucket['total'] = resp.hits.total
    for k in ('bright', 'dark', 'shadows_only', 'none'):
        if not k in preservation_bucket:
            preservation_bucket[k] = 0
    release_type_bucket = agg_to_dict(resp.aggregations.release_type)
    stats = {
        'ident': ident,
        'issnl': issnl,
        'total': resp.hits.total,
        'in_web': container_stats['in_web']['doc_count'],
        'in_kbart': container_stats['in_kbart']['doc_count'],
        'is_preserved': container_stats['is_preserved']['doc_count'],
        'preservation': preservation_bucket,
        'release_type': release_type_bucket,
    }

    return stats

def get_elastic_container_histogram_legacy(ident) -> List:
    """
    Fetches a stacked histogram of {year, in_ia}. This is for the older style
    of coverage graph (SVG or JSON export). This function should be DEPRECATED
    to be removed in the near future.

    Filters to the past 500 years (at most), or about 1000 values.

    Returns a list of tuples:
        (year, in_ia, count)
    """

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search = search.query(
        'bool',
        must=[
            Q("range", release_year={
                "gte": datetime.datetime.today().year - 499,
                "lte": datetime.datetime.today().year,
            }),
        ],
        filter=[
            Q("bool", minimum_should_match=1, should=[
                Q("match", container_id=ident),
            ]),
        ],
    )
    search.aggs.bucket(
        'year_in_ia',
        'composite',
        size=1000,
        sources=[
            {"year": {
                "histogram": {
                    "field": "release_year",
                    "interval": 1,
                },
            }},
            {"in_ia": {
                "terms": {
                    "field": "in_ia",
                },
            }},
        ],
    )
    search = search[:0]

    search = search.params(request_cache='true')
    resp = wrap_es_execution(search)

    buckets = resp.aggregations.year_in_ia.buckets
    vals = [(int(h['key']['year']), h['key']['in_ia'], h['doc_count'])
            for h in buckets]
    vals = sorted(vals)
    return vals


def get_elastic_preservation_by_year(query) -> List[dict]:
    """
    Fetches a stacked histogram of {year, preservation}.

    Preservation has 4 potential values; this function filters to the past 250
    years (at most), or about 1000 values.

    Returns a list of dicts, sorted by year, with keys/values like:

        {year (int), bright (int), dark (int), shadows_only (int), none (int)}
    """

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    if query.q not in [None, "*"]:
        search = search.query(
            "query_string",
            query=query.q,
            default_operator="AND",
            analyze_wildcard=True,
            allow_leading_wildcard=False,
            lenient=True,
            fields=[
                "biblio",
            ],
        )
    if query.container_id:
        search = search.filter(
            "term",
            container_id=query.container_id,
        )
    search = search.filter(
        "range",
        release_year={
            "gte": datetime.datetime.today().year - 249,
            "lte": datetime.datetime.today().year,
        },
    )

    search.aggs.bucket(
        'year_preservation',
        'composite',
        size=1500,
        sources=[
            {"year": {
                "histogram": {
                    "field": "release_year",
                    "interval": 1,
                },
            }},
            {"preservation": {
                "terms": {
                    "field": "preservation",
                },
            }},
        ],
    )
    search = search[:0]
    search = search.params(request_cache='true')
    resp = wrap_es_execution(search)

    buckets = resp.aggregations.year_preservation.buckets
    year_nums = set([int(h['key']['year']) for h in buckets])
    year_dicts = dict()
    for num in range(min(year_nums), max(year_nums)+1):
        year_dicts[num] = dict(year=num, bright=0, dark=0, shadows_only=0, none=0)
    for row in buckets:
        year_dicts[int(row['key']['year'])][row['key']['preservation']] = int(row['doc_count'])
    return sorted(year_dicts.values(), key=lambda x: x['year'])


def get_elastic_preservation_by_date(query) -> List[dict]:
    """
    Fetches a stacked histogram of {date, preservation}.

    Preservation has 4 potential values; this function filters to the past 250
    years (at most), or about 1000 values.

    Returns a list of dicts, sorted by date, with keys/values like:

        {date (str), bright (int), dark (int), shadows_only (int), none (int)}
    """

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    if query.q not in [None, "*"]:
        search = search.query(
            "query_string",
            query=query.q,
            default_operator="AND",
            analyze_wildcard=True,
            allow_leading_wildcard=False,
            lenient=True,
            fields=[
                "biblio",
            ],
        )
    if query.container_id:
        search = search.filter(
            "term",
            container_id=query.container_id,
        )
    date_today = datetime.date.today()
    start_date = date_today - datetime.timedelta(days=60)
    end_date = date_today + datetime.timedelta(days=1)
    search = search.filter(
        "range", release_date=dict(
            gte=str(start_date),
            lte=str(end_date),
        )
    )

    search.aggs.bucket(
        'date_preservation',
        'composite',
        size=1500,
        sources=[
            {"date": {
                "histogram": {
                    "field": "release_date",
                    "interval": 1,
                },
            }},
            {"preservation": {
                "terms": {
                    "field": "preservation",
                },
            }},
        ],
    )
    search = search[:0]
    search = search.params(request_cache='true')
    resp = wrap_es_execution(search)

    buckets = resp.aggregations.date_preservation.buckets
    date_dicts = dict()
    this_date = start_date
    while this_date <= end_date:
        date_dicts[str(this_date)] = dict(date=str(this_date), bright=0, dark=0, shadows_only=0, none=0)
        this_date = this_date + datetime.timedelta(days=1)
    for row in buckets:
        date_dicts[row['key']['date'][0:10]][row['key']['preservation']] = int(row['doc_count'])
    return sorted(date_dicts.values(), key=lambda x: x['date'])

def get_elastic_container_preservation_by_volume(container_id: str) -> List[dict]:
    """
    Fetches a stacked histogram of {volume, preservation}.

    Currently only includes volume numbers which are simple integers (all chars
    are digits).

    Returns a list of dicts, sorted by volume, with keys/values like:

        {year (int), bright (int), dark (int), shadows_only (int), none (int)}
    """

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    search = search.query(
        'bool',
        filter=[
            Q("bool", must=[
                Q("match", container_id=container_id),
                Q("exists", field="volume"),
            ]),
        ],
    )
    search.aggs.bucket(
        'volume_preservation',
        'composite',
        size=1500,
        sources=[
            {"volume": {
                "terms": {
                    "field": "volume",
                },
            }},
            {"preservation": {
                "terms": {
                    "field": "preservation",
                },
            }},
        ],
    )
    search = search[:0]
    search = search.params(request_cache='true')
    resp = wrap_es_execution(search)

    buckets = resp.aggregations.volume_preservation.buckets
    volume_nums = set([int(h['key']['volume']) for h in buckets if h['key']['volume'].isdigit()])
    volume_dicts = dict()
    for num in range(min(volume_nums), max(volume_nums)+1):
        volume_dicts[num] = dict(volume=num, bright=0, dark=0, shadows_only=0, none=0)
    for row in buckets:
        if row['key']['volume'].isdigit():
            volume_dicts[int(row['key']['volume'])][row['key']['preservation']] = int(row['doc_count'])
    return sorted(volume_dicts.values(), key=lambda x: x['volume'])

def get_elastic_preservation_by_type(query: ReleaseQuery) -> List[dict]:
    """
    Fetches preservation coverage by release type

    Returns a list of dicts, sorted by total count, with keys/values like:

        {year (int), bright (int), dark (int), shadows_only (int), none (int)}
    """

    search = Search(using=app.es_client, index=app.config['ELASTICSEARCH_RELEASE_INDEX'])
    if query.q not in [None, "*"]:
        search = search.query(
            "query_string",
            query=query.q,
            default_operator="AND",
            analyze_wildcard=True,
            allow_leading_wildcard=False,
            lenient=True,
            fields=[
                "biblio",
            ],
        )
    if query.container_id:
        search = search.query(
            'bool',
            filter=[
                Q("bool", must=[
                    Q("match", container_id=query.container_id),
                ]),
            ],
        )
    if query.recent:
        date_today = datetime.date.today()
        start_date = str(date_today - datetime.timedelta(days=60))
        end_date = str(date_today + datetime.timedelta(days=1))
        search = search.filter("range", release_date=dict(gte=start_date, lte=end_date))
    search.aggs.bucket(
        'type_preservation',
        'composite',
        size=1500,
        sources=[
            {"release_type": {
                "terms": {
                    "field": "release_type",
                },
            }},
            {"preservation": {
                "terms": {
                    "field": "preservation",
                },
            }},
        ],
    )
    search = search[:0]
    search = search.params(request_cache='true')
    resp = wrap_es_execution(search)

    buckets = resp.aggregations.type_preservation.buckets
    type_set = set([h['key']['release_type'] for h in buckets])
    type_dicts = dict()
    for k in type_set:
        type_dicts[k] = dict(release_type=k, bright=0, dark=0, shadows_only=0, none=0, total=0)
    for row in buckets:
        type_dicts[row['key']['release_type']][row['key']['preservation']] = int(row['doc_count'])
    for k in type_set:
        for p in ('bright', 'dark', 'shadows_only', 'none'):
            type_dicts[k]['total'] += type_dicts[k][p]
    return sorted(type_dicts.values(), key=lambda x: x['total'], reverse=True)
